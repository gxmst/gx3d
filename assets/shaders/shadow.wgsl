// Depth-only shadow pass.

struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
}

struct ObjectUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> shadow: ShadowUniforms;

@group(1) @binding(0)
var<uniform> object: ObjectUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    return shadow.light_view_proj * object.model * vec4<f32>(in.position, 1.0);
}
