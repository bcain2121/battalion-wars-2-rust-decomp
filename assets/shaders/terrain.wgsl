// Original terrain textures/UV data with smooth splat blending and hemisphere lighting.
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var texture_a: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var sampler_a: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var texture_b: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var sampler_b: sampler;

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let a = textureSample(texture_a, sampler_a, in.uv);
    let b = textureSample(texture_b, sampler_b, in.uv_b);
    let raw_blend = clamp(in.color.a * b.a, 0.0, 1.0);
    let blend = smoothstep(0.02, 0.98, raw_blend);
    let terrain_color = mix(a.rgb, b.rgb, blend);
    let n = normalize(in.world_normal);
    let sun_dir = normalize(vec3<f32>(0.42, 0.82, 0.34));
    let sun = max(dot(n, sun_dir), 0.0);
    let sky_hemi = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    let ambient = mix(vec3<f32>(0.68, 0.72, 0.78), vec3<f32>(0.92, 0.95, 1.0), sky_hemi);
    let vcol = vec3<f32>(0.74) + in.color.rgb * 1.16;
    let shade = (ambient * 0.82 + vec3<f32>(1.0, 0.96, 0.88) * (sun * 0.42)) * vcol;
    var out: FragmentOutput;
    out.color = vec4<f32>(terrain_color * shade, 1.0);
    return out;
}
