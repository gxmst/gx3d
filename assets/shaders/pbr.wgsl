// GxEngine PBR Shader with Multi-Light Support

const MAX_LIGHTS: u32 = 8u;
// Must match renderer::SHADOW_MAP_SIZE (guarded by a Rust unit test).
const SHADOW_MAP_SIZE: f32 = 2048.0;

struct Light {
    position: vec3<f32>,
    light_type: u32,    // 0=directional, 1=point, 2=spot
    color: vec3<f32>,
    intensity: f32,
    direction: vec3<f32>,
    range: f32,
}

// Global uniforms (binding 0)
struct GlobalUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    time: f32,
    num_lights: u32,
    _padding: vec2<f32>,
    lights: array<Light, MAX_LIGHTS>,
    light_space_matrix: mat4x4<f32>,
}

// Material uniforms (binding 1)
struct MaterialUniforms {
    albedo_factor: vec4<f32>,
    metallic: f32,
    roughness: f32,
    has_albedo_map: u32,
    has_normal_map: u32,
    emissive_factor: vec3<f32>,
    has_emissive_map: u32,
    has_metallic_roughness_map: u32,
    is_water: u32,
    // Scalar padding, NOT vec2<u32>: keeps WGSL size at 64 to match Rust.
    _padding0: u32,
    _padding1: u32,
}

// Object uniforms (binding 2)
struct ObjectUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> global: GlobalUniforms;

@group(0) @binding(1)
var t_irradiance: texture_cube<f32>;

@group(0) @binding(2)
var t_prefilter: texture_cube<f32>;

@group(0) @binding(3)
var t_brdf_lut: texture_2d<f32>;

@group(0) @binding(4)
var s_ibl: sampler;

@group(0) @binding(5)
var s_brdf: sampler;

@group(0) @binding(6)
var t_shadow: texture_depth_2d;

@group(0) @binding(7)
var s_shadow: sampler_comparison;

@group(1) @binding(0)
var<uniform> material: MaterialUniforms;

@group(1) @binding(1)
var t_albedo: texture_2d<f32>;

@group(1) @binding(2)
var t_normal: texture_2d<f32>;

@group(1) @binding(3)
var s_material: sampler;

@group(1) @binding(4)
var t_emissive: texture_2d<f32>;

@group(1) @binding(5)
var t_metallic_roughness: texture_2d<f32>;

@group(2) @binding(0)
var<uniform> object: ObjectUniforms;

// Vertex input
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
}

// Vertex output
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_position = object.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_position.xyz;
    out.world_normal = normalize((object.normal_matrix * vec4<f32>(in.normal, 0.0)).xyz);
    out.tangent = normalize((object.model * vec4<f32>(in.tangent.xyz, 0.0)).xyz);
    out.bitangent = cross(out.world_normal, out.tangent) * in.tangent.w;
    out.uv = in.uv;
    out.clip_position = global.view_proj * world_position;

    return out;
}

// Fragment output
struct FragmentOutput {
    @location(0) color: vec4<f32>,
}

const PI: f32 = 3.14159265359;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    let nom = a2;
    var denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    denom = PI * denom * denom;
    return nom / denom;
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    let nom = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;
    return nom / denom;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
    return ggx1 * ggx2;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return f0 + (max(vec3<f32>(1.0 - roughness), f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn sample_ibl_diffuse(n: vec3<f32>, albedo: vec3<f32>, metallic: f32, v: vec3<f32>, f0: vec3<f32>) -> vec3<f32> {
    let irradiance = textureSample(t_irradiance, s_ibl, n).rgb;
    let k_d = (vec3<f32>(1.0) - fresnel_schlick_roughness(max(dot(n, v), 0.0), f0, 0.0)) * (1.0 - metallic);
    return albedo * irradiance * k_d;
}

fn sample_ibl_specular(n: vec3<f32>, v: vec3<f32>, roughness: f32, f0: vec3<f32>) -> vec3<f32> {
    let r = reflect(-v, n);
    let prefiltered = textureSampleLevel(t_prefilter, s_ibl, r, roughness * 4.0).rgb;
    let n_dot_v = max(dot(n, v), 0.0);
    let brdf = textureSample(t_brdf_lut, s_brdf, vec2<f32>(n_dot_v, roughness)).rg;
    return prefiltered * (f0 * brdf.x + brdf.y);
}

// --- Procedural surface detail -------------------------------------------
// World-space value noise used to break up the "perfect plastic" look of
// flat-colored materials: albedo blotches, roughness grain, and a micro
// normal perturbation. Strength scales with roughness and vanishes on
// metals, so mirrors and polished props stay clean.

fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn value_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n000 = hash31(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));
    return mix(
        mix(mix(n000, n100, u.x), mix(n010, n110, u.x), u.y),
        mix(mix(n001, n101, u.x), mix(n011, n111, u.x), u.y),
        u.z,
    );
}

fn fbm(p: vec3<f32>) -> f32 {
    var total = 0.0;
    var amplitude = 0.5;
    var q = p;
    for (var i = 0; i < 3; i++) {
        total += value_noise(q) * amplitude;
        q = q * 2.17;
        amplitude *= 0.5;
    }
    return total;
}

// Gradient of the detail bump height field, via central differences. Used to
// tilt the shading normal slightly so flat walls pick up surface relief.
fn detail_bump_gradient(p: vec3<f32>) -> vec3<f32> {
    let e = 0.06;
    let dx = value_noise(p + vec3<f32>(e, 0.0, 0.0)) - value_noise(p - vec3<f32>(e, 0.0, 0.0));
    let dy = value_noise(p + vec3<f32>(0.0, e, 0.0)) - value_noise(p - vec3<f32>(0.0, e, 0.0));
    let dz = value_noise(p + vec3<f32>(0.0, 0.0, e)) - value_noise(p - vec3<f32>(0.0, 0.0, e));
    return vec3<f32>(dx, dy, dz) / (2.0 * e);
}

fn sample_shadow_pcf(world_pos: vec3<f32>, n_dot_l: f32) -> f32 {
    let light_pos = global.light_space_matrix * vec4<f32>(world_pos, 1.0);
    let proj_coords = light_pos.xyz / light_pos.w;
    let uv = proj_coords.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let current_depth = proj_coords.z;

    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || current_depth > 1.0) {
        return 1.0;
    }

    // Slope-scaled bias: surfaces nearly parallel to the light need more
    // offset to avoid acne, while facing surfaces keep contact shadows tight.
    let bias = clamp(0.0035 * (1.0 - n_dot_l), 0.0008, 0.0035);

    let texel_size = 1.0 / SHADOW_MAP_SIZE;
    var shadow = 0.0;
    for (var x: i32 = -2; x <= 2; x = x + 1) {
        for (var y: i32 = -2; y <= 2; y = y + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow = shadow + textureSampleCompare(t_shadow, s_shadow, uv + offset, current_depth - bias);
        }
    }
    return shadow / 25.0;
}

fn calculate_light_radiance(light: Light, world_pos: vec3<f32>, n: vec3<f32>, v: vec3<f32>, f0: vec3<f32>, albedo: vec3<f32>, metallic: f32, roughness: f32) -> vec3<f32> {
    var l: vec3<f32>;
    var radiance: vec3<f32>;
    let attenuation = 1.0;
    var shadow = 1.0;

    if (light.light_type == 0u) {
        // Directional light
        l = normalize(-light.direction);
        radiance = light.color * light.intensity;
        shadow = sample_shadow_pcf(world_pos, max(dot(n, l), 0.0));
    } else if (light.light_type == 1u) {
        // Point light
        let to_light = light.position - world_pos;
        let distance = length(to_light);
        if (distance > light.range) {
            return vec3<f32>(0.0);
        }
        l = normalize(to_light);
        let falloff = 1.0 - smoothstep(0.0, light.range, distance);
        radiance = light.color * light.intensity * falloff / (distance * distance + 1.0);
    } else {
        // Spot light (treat as point for now)
        let to_light = light.position - world_pos;
        let distance = length(to_light);
        if (distance > light.range) {
            return vec3<f32>(0.0);
        }
        l = normalize(to_light);
        let falloff = 1.0 - smoothstep(0.0, light.range, distance);
        radiance = light.color * light.intensity * falloff / (distance * distance + 1.0);
    }

    let h = normalize(v + l);

    let ndf = distribution_ggx(n, h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let numerator = ndf * g * f;
    let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
    let specular = numerator / denominator;

    let ks = f;
    var kd = vec3<f32>(1.0) - ks;
    kd *= 1.0 - metallic;

    let n_dot_l = max(dot(n, l), 0.0);

    return (kd * albedo / PI + specular) * radiance * n_dot_l * shadow;
}

// Water: fresnel-weighted sky reflection over a depth-tinted body color,
// animated micro-ripples on top of the CPU-side swell, and whitecap foam on
// steep crests. Transparency comes from the alpha-blend pipeline; alpha rises
// with fresnel so grazing angles read as a solid mirror while looking straight
// down stays see-through.
fn shade_water(in: VertexOutput, geo_n: vec3<f32>) -> FragmentOutput {
    var out: FragmentOutput;
    let v = normalize(global.camera_pos.xyz - in.world_position);

    // Micro-ripples: two scrolling noise layers perturb the swell normal.
    let t = global.time;
    let p1 = in.world_position * 1.9 + vec3<f32>(t * 0.55, 0.0, t * 0.38);
    let p2 = in.world_position * 4.3 + vec3<f32>(-t * 0.42, 0.0, t * 0.61);
    let ripple = vec3<f32>(
        value_noise(p1) - 0.5 + (value_noise(p2) - 0.5) * 0.5,
        0.0,
        value_noise(p1 + vec3<f32>(31.7, 0.0, 17.3)) - 0.5
            + (value_noise(p2 + vec3<f32>(11.1, 0.0, 47.9)) - 0.5) * 0.5,
    );
    var n = normalize(geo_n + ripple * 0.35);

    // Fresnel between the deep-water body color and the reflected sky.
    let n_dot_v = max(dot(n, v), 0.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - n_dot_v, 5.0);

    let deep_color = material.albedo_factor.rgb;
    let shallow_color = deep_color * 2.6 + vec3<f32>(0.02, 0.10, 0.10);
    // Facing ratio stands in for view depth: grazing looks deep, straight
    // down looks shallow/bright.
    let body = mix(deep_color, shallow_color, n_dot_v * 0.55);

    let reflection = sample_ibl_specular(n, v, 0.06, vec3<f32>(0.02));
    // Sun glint: specular from the first directional light.
    var glint = vec3<f32>(0.0);
    let light_count = min(global.num_lights, MAX_LIGHTS);
    for (var i: u32 = 0u; i < light_count; i++) {
        if (global.lights[i].light_type == 0u) {
            let l = normalize(-global.lights[i].direction);
            let h = normalize(v + l);
            let spec = pow(max(dot(n, h), 0.0), 240.0);
            glint += global.lights[i].color * global.lights[i].intensity * spec * 0.9;
        }
    }

    // Whitecap foam on crests: geometric normal tilt marks steep swell;
    // noise breaks the foam edge up.
    let steepness = 1.0 - geo_n.y;
    let crest = smoothstep(0.015, 0.05, steepness);
    let foam_noise = fbm(in.world_position * 1.3 + vec3<f32>(t * 0.3, 0.0, t * 0.22));
    let foam = crest * smoothstep(0.35, 0.65, foam_noise);

    var color = mix(body, reflection, fresnel) + glint;
    color = mix(color, vec3<f32>(0.92, 0.96, 0.97), foam * 0.85);

    // More opaque at grazing angles and under foam; clearest looking down.
    let alpha = clamp(0.55 + fresnel * 0.4 + foam * 0.45, 0.0, 0.97);
    out.color = vec4<f32>(color, alpha);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // Sample albedo
    var albedo = material.albedo_factor.rgb;
    if (material.has_albedo_map == 1u) {
        let albedo_tex = textureSample(t_albedo, s_material, in.uv).rgb;
        albedo = albedo_tex * material.albedo_factor.rgb;
    }

    // Sample normal map
    var n = normalize(in.world_normal);
    if (material.has_normal_map == 1u) {
        let normal_map = textureSample(t_normal, s_material, in.uv).xyz * 2.0 - 1.0;
        let tbn = mat3x3<f32>(normalize(in.tangent), normalize(in.bitangent), normalize(in.world_normal));
        n = normalize(tbn * normal_map);
    }

    // glTF metallic-roughness convention: G = roughness, B = metallic.
    var metallic = material.metallic;
    var roughness = material.roughness;
    if (material.has_metallic_roughness_map == 1u) {
        let mr = textureSample(t_metallic_roughness, s_material, in.uv);
        roughness = roughness * mr.g;
        metallic = metallic * mr.b;
    }
    roughness = clamp(roughness, 0.04, 1.0);

    // Procedural surface detail: strongest on rough dielectrics (concrete,
    // wood, plaster), absent on metals and polished surfaces, and skipped
    // when a real normal map is bound.
    let detail_strength = (1.0 - metallic) * smoothstep(0.3, 0.75, roughness);
    if (detail_strength > 0.01) {
        // Large-scale blotches darken/lighten the albedo like grime and
        // sun bleaching; fine grain modulates roughness like surface dust.
        let blotch = fbm(in.world_position * 0.9);
        let grain = value_noise(in.world_position * 21.0);
        albedo *= 1.0 + (blotch - 0.5) * 0.34 * detail_strength;
        roughness = clamp(roughness + (grain - 0.5) * 0.22 * detail_strength, 0.04, 1.0);
        if (material.has_normal_map == 0u) {
            // Micro relief: tilt the normal by the bump gradient projected
            // onto the surface plane.
            let grad = detail_bump_gradient(in.world_position * 3.4);
            let tangent_grad = grad - n * dot(grad, n);
            n = normalize(n - tangent_grad * 0.10 * detail_strength);
        }
    }

    let v = normalize(global.camera_pos.xyz - in.world_position);

    var f0 = vec3<f32>(0.04);
    f0 = mix(f0, albedo, metallic);

    // Accumulate lighting from all lights
    var lo = vec3<f32>(0.0);
    let light_count = min(global.num_lights, MAX_LIGHTS);

    for (var i: u32 = 0u; i < light_count; i++) {
        lo += calculate_light_radiance(global.lights[i], in.world_position, n, v, f0, albedo, metallic, roughness);
    }

    // If no lights, add a default directional light for visibility
    if (light_count == 0u) {
        let default_light = Light(
            vec3<f32>(0.0),
            0u,
            vec3<f32>(1.0, 0.95, 0.9),
            3.0,
            vec3<f32>(1.0, 1.0, 1.0),
            1000.0,
        );
        lo += calculate_light_radiance(default_light, in.world_position, n, v, f0, albedo, metallic, roughness);
    }

    let ibl_diffuse = sample_ibl_diffuse(n, albedo, metallic, v, f0);
    let ibl_specular = sample_ibl_specular(n, v, roughness, f0);
    let ambient = ibl_diffuse + ibl_specular + vec3<f32>(0.02) * albedo;
    var color = ambient + lo;

    // Emission added before tone mapping so bright materials can bloom later.
    var emission = material.emissive_factor;
    if (material.has_emissive_map == 1u) {
        emission = emission * textureSample(t_emissive, s_material, in.uv).rgb;
    }
    color = color + emission;

    // Keep linear HDR output for bloom and post-process tone mapping. Alpha
    // passes through for the transparent pipeline; the opaque pipeline's
    // REPLACE blend ignores it.
    out.color = vec4<f32>(color, material.albedo_factor.a);
    return out;
}
