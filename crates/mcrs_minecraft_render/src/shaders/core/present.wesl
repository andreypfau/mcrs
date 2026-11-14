import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;
import package::shaders::include::finish::to_target;

@group(0) @binding(0) var world: texture_2d<f32>;

@fragment
fn present(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    return vec4<f32>(to_target(textureLoad(world, pixel, 0).rgb), 1.0);
}
