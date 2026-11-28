fn lightmap_sample(levels: texture_2d<f32>, level_sampler: sampler, block: f32, sky: f32) -> vec3<f32> {
    let uv = clamp(
        vec2<f32>(block, sky) / 256.0 + 0.5 / 16.0,
        vec2<f32>(0.5 / 16.0),
        vec2<f32>(15.5 / 16.0),
    );
    return textureSampleLevel(levels, level_sampler, uv, 0.0).rgb;
}
