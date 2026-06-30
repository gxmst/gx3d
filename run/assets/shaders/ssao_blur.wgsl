// 5x5 box blur for the SSAO output.

@group(0) @binding(0)
var t_ssao: texture_2d<f32>;

@group(0) @binding(1)
var s_ssao: sampler;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    let texel = 1.0 / vec2<f32>(textureDimensions(t_ssao, 0));
    var result = 0.0;
    for (var x: i32 = -2; x <= 2; x = x + 1) {
        for (var y: i32 = -2; y <= 2; y = y + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            result = result + textureSample(t_ssao, s_ssao, in.uv + offset).r;
        }
    }
    return result / 25.0;
}
