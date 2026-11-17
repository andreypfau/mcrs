import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;
import mcrs_minecraft_render::shaders::include::depth::depth_at;
import mcrs_minecraft_render::shaders::include::show_layout::{nearest, source};

const DEPTH_OCTAVES: f32 = 14.0;

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
    return vec4<f32>(vec3<f32>(grey), 1.0);
}
