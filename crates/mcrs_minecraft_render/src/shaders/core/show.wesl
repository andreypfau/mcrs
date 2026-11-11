import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;
import package::shaders::include::depth::depth_at;
import package::shaders::include::finish::to_target;

// Reverse-Z depth is the near plane over the distance, so each octave of distance is one step of
// log2(depth), and 14 octaves span from the near plane past the farthest render distance.
const DEPTH_OCTAVES: f32 = 14.0;

// The view depth is read as a float texture too: GLSL loads no texel of a depth texture.
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;

fn shown_depth(pixel: vec2<u32>) -> f32 {
    @if(PYRAMID) {
        let size = textureDimensions(source, 0);
        return textureLoad(source, min(pixel / 2u, size - 1u), 0).r;
    } @else {
        return depth_at(source, nearest, vec2<i32>(pixel));
    }
}

@fragment
fn show(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let depth = max(shown_depth(vec2<u32>(in.position.xy)), exp2(-DEPTH_OCTAVES));
    let grey = clamp(1.0 + log2(depth) / DEPTH_OCTAVES, 0.0, 1.0);
    return vec4<f32>(to_target(vec3<f32>(grey)), 1.0);
}
