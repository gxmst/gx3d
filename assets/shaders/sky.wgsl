struct SkyUniform { view_projection: mat4x4<f32>, sun_direction: vec4<f32>, sky_params: vec4<f32> };
@group(0) @binding(0) var<uniform> sky: SkyUniform;
struct In { @location(0) position: vec3<f32> };
struct Out { @builtin(position) position: vec4<f32>, @location(0) direction: vec3<f32> };
@vertex fn vs_main(input: In) -> Out {
    var out: Out;
    out.position = sky.view_projection * vec4<f32>(input.position, 1.0);
    out.direction = normalize(input.position);
    return out;
}
fn hash(p: vec2<f32>) -> f32 { return fract(sin(dot(p,vec2<f32>(127.1,311.7)))*43758.5453); }
fn noise(p: vec2<f32>) -> f32 { let i=floor(p);let f=fract(p);let u=f*f*(3.0-2.0*f);return mix(mix(hash(i),hash(i+vec2<f32>(1,0)),u.x),mix(hash(i+vec2<f32>(0,1)),hash(i+vec2<f32>(1,1)),u.x),u.y); }
fn fbm(p0: vec2<f32>) -> f32 { var p=p0;var v=0.0;var a=0.5;for(var i=0;i<5;i=i+1){v+=noise(p)*a;p=p*2.03+vec2<f32>(17.1,9.2);a*=0.5;}return v; }
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
    let dir=normalize(input.direction);let daylight=sky.sky_params.y;let horizon=pow(1.0-max(dir.y,0.0),3.0);
    var color=mix(vec3<f32>(0.004,0.01,0.03),mix(vec3<f32>(0.055,0.25,0.68),vec3<f32>(0.62,0.78,0.95),horizon),daylight);
    color+=vec3<f32>(1.0,0.72,0.36)*pow(max(dot(dir,normalize(sky.sun_direction.xyz)),0.0),700.0)*(2.5*daylight);
    if(dir.y>0.02&&daylight>0.08){let uv=dir.xz/max(dir.y+0.24,0.08)*0.75+vec2<f32>(sky.sky_params.x*0.003,0.0);let c=smoothstep(0.54,0.72,fbm(uv));color=mix(color,vec3<f32>(0.92,0.95,1.0)*(0.65+0.35*daylight),c*0.72*smoothstep(0.02,0.25,dir.y));}
    return vec4<f32>(color,1.0);
}
