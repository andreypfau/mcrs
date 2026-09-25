#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import mcrs_minecraft_client::deferred::octahedral_decode
#import mcrs_minecraft_client::finish::to_target

@group(2) @binding(0) var albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var normal_motion: texture_2d<f32>;
@group(2) @binding(2) var light: texture_2d<f32>;
@group(2) @binding(3) var depth: texture_depth_2d;

// Block and sky light are stored in sixteenths of a level.
const FULL_LIGHT: f32 = 240.0;

@fragment
fn show(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    if (textureLoad(depth, pixel, 0) == 0.0) {
        discard;
    }
#ifdef SHOW_ALBEDO
    let colour = textureLoad(albedo_ao, pixel, 0).rgb;
#else ifdef SHOW_AO
    let colour = vec3<f32>(textureLoad(albedo_ao, pixel, 0).a);
#else ifdef SHOW_NORMAL
    let colour = octahedral_decode(textureLoad(normal_motion, pixel, 0).xy) * 0.5 + 0.5;
#else ifdef SHOW_BLOCK_LIGHT
    let colour = vec3<f32>(textureLoad(light, pixel, 0).g / FULL_LIGHT);
#else ifdef SHOW_SKY_LIGHT
    let colour = vec3<f32>(textureLoad(light, pixel, 0).b / FULL_LIGHT);
#endif
    return vec4<f32>(to_target(colour), 1.0);
}
