// Post-process: ACES tone mapping + gamma correction.

struct PostProcessUniforms {
    exposure: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> params: PostProcessUniforms;

@group(0) @binding(1)
var t_hdr: texture_2d<f32>;

@group(0) @binding(2)
var s_hdr: sampler;

@group(0) @binding(3)
var t_bloom: texture_2d<f32>;

@group(0) @binding(4)
var s_bloom: sampler;

@group(0) @binding(5)
var t_ao: texture_2d<f32>;

@group(0) @binding(6)
var s_ao: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

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

fn aces_tone_map(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr = textureSample(t_hdr, s_hdr, in.uv).rgb;
    let bloom = textureSample(t_bloom, s_bloom, in.uv).rgb;
    let ao = textureSample(t_ao, s_ao, in.uv).r;
    let softened_ao = mix(1.0, ao, 0.38);
    let exposed = (hdr + bloom * 0.16) * params.exposure.x * softened_ao;
    let mapped = aces_tone_map(exposed);
    // The swapchain uses an sRGB format, so conversion is performed by the
    // render target. Applying gamma here as well caused the washed film veil.
    return vec4<f32>(mapped, 1.0);
}
