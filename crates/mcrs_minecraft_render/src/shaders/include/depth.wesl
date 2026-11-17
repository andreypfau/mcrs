fn depth_at(depth: texture_2d<f32>, nearest: sampler, pixel: vec2<i32>) -> f32 {
    let uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(textureDimensions(depth, 0));
    return textureSampleLevel(depth, nearest, uv, 0.0).r;
}
