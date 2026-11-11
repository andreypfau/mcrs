/// The depth at `pixel`, sampled at its centre rather than loaded. GLSL ES calls a depth texture
/// that no comparing sampler reads incomplete unless its filters are nearest, and loads zero from
/// it; wgpu leaves a depth texture's own filters at their defaults, so the nearest sampler stands
/// in for them.
fn depth_at(depth: texture_2d<f32>, nearest: sampler, pixel: vec2<i32>) -> f32 {
    let uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(textureDimensions(depth, 0));
    return textureSampleLevel(depth, nearest, uv, 0.0).r;
}
