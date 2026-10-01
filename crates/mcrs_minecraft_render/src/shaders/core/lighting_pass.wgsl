#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import mcrs_minecraft_client::deferred::octahedral_decode
#import mcrs_minecraft_client::finish::to_target
#ifdef LIGHT_TINT
#import mcrs_minecraft_client::volume::{shade_tinted, volume_tint}
#else
#import mcrs_minecraft_client::deferred::shade
#endif

struct Reconstruction {
    relative_from_clip: mat4x4<f32>,
    viewport: vec2<f32>,
}

@group(2) @binding(0) var albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var normal_motion: texture_2d<f32>;
@group(2) @binding(2) var light: texture_2d<f32>;
@group(2) @binding(3) var depth: texture_depth_2d;
@group(2) @binding(4) var<uniform> reconstruction: Reconstruction;

/// Against the origin of the camera's section, as terrain is drawn.
fn relative_position(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(pixel) + 0.5) / reconstruction.viewport;
    let ndc = uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    let p = reconstruction.relative_from_clip * vec4<f32>(ndc, depth, 1.0);
    return p.xyz / p.w;
}

@fragment
fn lighting(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let d = textureLoad(depth, pixel, 0);
    if (d == 0.0) {
        discard;
    }
#ifdef UNLIT
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
#else
    let a = textureLoad(albedo_ao, pixel, 0);
    let l = textureLoad(light, pixel, 0);
#ifdef LIGHTING_TERM
    let albedo = vec3<f32>(1.0);
#else
    let albedo = a.rgb;
#endif
#ifdef LIGHT_TINT
    let normal = octahedral_decode(textureLoad(normal_motion, pixel, 0).xy);
    let tint = volume_tint(relative_position(pixel, d), normal);
    return vec4<f32>(to_target(shade_tinted(albedo, a.a, l.g, l.b, tint)), 1.0);
#else
    return vec4<f32>(to_target(shade(albedo, a.a, l.g, l.b)), 1.0);
#endif
#endif
}
