// Screen-space ambient occlusion from linearized depth.

struct SsaoParams {
    projection: mat4x4<f32>,
    inv_projection: mat4x4<f32>,
    noise_scale: vec2<f32>,
    radius: f32,
    bias: f32,
    strength: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

const KERNEL_SIZE: u32 = 64u;

@group(0) @binding(0)
var<uniform> params: SsaoParams;

@group(0) @binding(1)
var<uniform> kernel: array<vec4<f32>, KERNEL_SIZE>;

@group(0) @binding(2)
var t_depth: texture_depth_2d;

@group(0) @binding(3)
var s_depth: sampler;

@group(0) @binding(4)
var t_noise: texture_2d<f32>;

@group(0) @binding(5)
var s_noise: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    let p = positions[vertex_index];
    out.clip_position = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

fn position_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    // Match the fullscreen-quad UV convention used by post_process.wgsl.
    let clip = vec4<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0), depth, 1.0);
    let view_h = params.inv_projection * clip;
    return view_h.xyz / view_h.w;
}

fn normal_from_depth(uv: vec2<f32>) -> vec3<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(t_depth, 0));
    let c = textureSample(t_depth, s_depth, uv);
    let l = textureSample(t_depth, s_depth, uv + vec2<f32>(-texel.x, 0.0));
    let r = textureSample(t_depth, s_depth, uv + vec2<f32>(texel.x, 0.0));
    let u = textureSample(t_depth, s_depth, uv + vec2<f32>(0.0, -texel.y));
    let d = textureSample(t_depth, s_depth, uv + vec2<f32>(0.0, texel.y));

    let pC = position_from_depth(uv, c);
    let pL = position_from_depth(uv + vec2<f32>(-texel.x, 0.0), l);
    let pR = position_from_depth(uv + vec2<f32>(texel.x, 0.0), r);
    let pU = position_from_depth(uv + vec2<f32>(0.0, -texel.y), u);
    let pD = position_from_depth(uv + vec2<f32>(0.0, texel.y), d);

    let n1 = cross(pR - pC, pU - pC);
    let n2 = cross(pL - pC, pD - pC);
    return normalize(n1 + n2);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    let depth = textureSample(t_depth, s_depth, in.uv);
    let frag_pos = position_from_depth(in.uv, depth);
    let normal = normal_from_depth(in.uv);

    let noise_vec = textureSample(t_noise, s_noise, in.uv * params.noise_scale).xyz;
    let tangent = normalize(noise_vec - normal * dot(noise_vec, normal));
    let bitangent = cross(normal, tangent);
    let tbn = mat3x3<f32>(tangent, bitangent, normal);

    var occlusion = 0.0;
    for (var i: u32 = 0u; i < KERNEL_SIZE; i = i + 1u) {
        let sample_pos = frag_pos + (tbn * kernel[i].xyz) * params.radius;

        let offset = params.projection * vec4<f32>(sample_pos, 1.0);
        let offset_ndc = offset.xyz / offset.w;
        let sample_uv = vec2<f32>(
            offset_ndc.x * 0.5 + 0.5,
            -offset_ndc.y * 0.5 + 0.5,
        );

        let sample_depth = textureSample(t_depth, s_depth, sample_uv);
        let sample_view_z = position_from_depth(sample_uv, sample_depth).z;

        let range_check = smoothstep(0.0, 1.0, params.radius / abs(frag_pos.z - sample_view_z));
        if (sample_view_z >= sample_pos.z + params.bias) {
            occlusion = occlusion + range_check;
        }
    }

    occlusion = 1.0 - (occlusion / f32(KERNEL_SIZE));
    return clamp(pow(occlusion, params.strength), 0.0, 1.0);
}
