// Separable Gaussian blur pass for bloom.

struct BlurUniforms {
    direction: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> blur: BlurUniforms;

@group(0) @binding(1)
var t_input: texture_2d<f32>;

@group(0) @binding(2)
var s_input: sampler;

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

const WEIGHTS: array<f32, 5> = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(t_input));
    let inv_size = 1.0 / tex_size;
    var result = textureSample(t_input, s_input, in.uv).rgb * WEIGHTS[0];
    for (var i: i32 = 1; i < 5; i = i + 1) {
        let offset = f32(i) * blur.direction * inv_size;
        result = result + textureSample(t_input, s_input, in.uv + offset).rgb * WEIGHTS[i];
        result = result + textureSample(t_input, s_input, in.uv - offset).rgb * WEIGHTS[i];
    }
    return vec4<f32>(result, 1.0);
}
