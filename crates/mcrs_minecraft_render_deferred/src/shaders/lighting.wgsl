#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import mcrs_minecraft_client::deferred::shade
#import mcrs_minecraft_client::finish::to_target

@group(2) @binding(0) var albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var normal_motion: texture_2d<f32>;
@group(2) @binding(2) var light: texture_2d<f32>;
@group(2) @binding(3) var depth: texture_depth_2d;

@fragment
fn lighting(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
#ifdef PARITY_MASK
    let sky = textureLoad(depth, pixel, 0) == 0.0;
    let uniform_corners = textureLoad(light, pixel, 0).a > 0.5;
    return vec4<f32>(vec3<f32>(f32(sky || uniform_corners)), 1.0);
#else
    if (textureLoad(depth, pixel, 0) == 0.0) {
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
    return vec4<f32>(to_target(shade(albedo, a.a, l.g, l.b)), 1.0);
#endif
#endif
}
