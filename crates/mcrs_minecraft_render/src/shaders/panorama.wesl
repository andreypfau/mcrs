import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;
import package::shaders::include::finish::{from_target, to_target};

@group(0) @binding(0) var<uniform> ray_from_ndc: mat4x4<f32>;
@group(0) @binding(1) var faces: texture_cube<f32>;
@group(0) @binding(2) var overlay: texture_2d<f32>;
@group(0) @binding(3) var linear: sampler;

@fragment
fn panorama(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let ray = (ray_from_ndc * vec4<f32>(ndc, 1.0, 0.0)).xyz * vec3<f32>(1.0, 1.0, -1.0);
    let face = from_target(textureSample(faces, linear, ray).rgb);
    let over = textureSample(overlay, linear, in.uv);
    return vec4<f32>(to_target(mix(face, from_target(over.rgb), over.a)), 1.0);
}

@group(0) @binding(4) var blur_source: texture_2d<f32>;

const BLUR_RADIUS: i32 = 5;

fn box_blur(pixel: vec2<i32>, step: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(blur_source)) - 1;
    var sum = vec4<f32>(0.0);
    for (var offset = -BLUR_RADIUS; offset <= BLUR_RADIUS; offset += 1) {
        let at = clamp(pixel + step * offset, vec2<i32>(0), last);
        let stored = textureLoad(blur_source, at, 0);
        sum += vec4<f32>(from_target(stored.rgb), stored.a);
    }
    let mean = sum / f32(2 * BLUR_RADIUS + 1);
    return vec4<f32>(to_target(mean.rgb), mean.a);
}

@fragment
fn blur_horizontal(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return box_blur(vec2<i32>(in.position.xy), vec2<i32>(1, 0));
}

@fragment
fn blur_vertical(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return box_blur(vec2<i32>(in.position.xy), vec2<i32>(0, 1));
}
