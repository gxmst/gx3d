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

fn interleaved_gradient_noise(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

fn srgb_to_linear(encoded: vec3<f32>) -> vec3<f32> {
    let low = encoded / 12.92;
    let high = pow((encoded + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, encoded <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr = textureSample(t_hdr, s_hdr, in.uv).rgb;
    let bloom = textureSample(t_bloom, s_bloom, in.uv).rgb;
    let ao = textureSample(t_ao, s_ao, in.uv).r;
    // AO weighs in a little harder than before; grounded contact shading is
    // most of what separates "clay render" from a lit space.
    let softened_ao = mix(1.0, ao, 0.52);
    let exposed = (hdr + bloom * 0.16) * params.exposure.x * softened_ao;
    var mapped = aces_tone_map(exposed);

    // Subtle grade: a touch of saturation and a soft vignette focus the eye
    // without reading as a filter.
    let luma = dot(mapped, vec3<f32>(0.2126, 0.7152, 0.0722));
    mapped = clamp(mix(vec3<f32>(luma), mapped, 1.07), vec3<f32>(0.0), vec3<f32>(1.0));
    let offset = in.uv - vec2<f32>(0.5, 0.5);
    let vignette = 1.0 - dot(offset, offset) * 0.34;
    mapped *= clamp(vignette, 0.0, 1.0);
    let dither = (interleaved_gradient_noise(in.clip_position.xy) - 0.5) / 255.0;
    // Dither by one encoded output step, then return to linear for the sRGB
    // render target. Linear-space noise is amplified heavily near black.
    let encoded = linear_to_srgb(mapped);
    let dithered = clamp(encoded + vec3<f32>(dither), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(srgb_to_linear(dithered), 1.0);
}
