#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import mcrs_minecraft_client::finish::to_target

// Reverse-Z depth is the near plane over the distance, so each octave of distance is one step of
// log2(depth), and 14 octaves span from the near plane past the farthest render distance.
const DEPTH_OCTAVES: f32 = 14.0;

#ifdef PYRAMID
@group(0) @binding(0) var source: texture_2d<f32>;

fn depth_at(pixel: vec2<u32>) -> f32 {
    let size = textureDimensions(source, 0);
    return textureLoad(source, min(pixel / 2u, size - 1u), 0).r;
}
#else
@group(0) @binding(0) var source: texture_depth_2d;

fn depth_at(pixel: vec2<u32>) -> f32 {
    return textureLoad(source, pixel, 0);
}
#endif

@fragment
fn show(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let depth = max(depth_at(vec2<u32>(in.position.xy)), exp2(-DEPTH_OCTAVES));
    let grey = clamp(1.0 + log2(depth) / DEPTH_OCTAVES, 0.0, 1.0);
    return vec4<f32>(to_target(vec3<f32>(grey)), 1.0);
}
