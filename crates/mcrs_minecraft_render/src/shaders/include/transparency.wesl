import package::shaders::include::depth::depth_at;
import package::shaders::include::finish::from_target;
import package::shaders::include::fog::{cloud_over, improved_fog_mix};

@group(2) @binding(6) var sky_image: texture_2d<f32>;
@group(2) @binding(7) var cloud_image: texture_2d<f32>;
@group(2) @binding(8) var cloud_depth: texture_2d<f32>;
@group(2) @binding(9) var cloud_nearest: sampler;

fn sky_texel(pixel: vec2<i32>) -> vec4<f32> {
    return textureLoad(sky_image, pixel, 0);
}

/// `cl` and `ca` of the cloud image in image values. The image holds them as the target blends.
fn cloud_at(pixel: vec2<i32>) -> vec4<f32> {
    let stored = textureLoad(cloud_image, pixel, 0);
    if (stored.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(from_target(stored.rgb / stored.a) * stored.a, stored.a);
}

fn cloud_depth_at(pixel: vec2<i32>) -> f32 {
    return depth_at(cloud_depth, cloud_nearest, pixel);
}

/// The fog of a face at `pixel` and the reverse-Z `depth`, where a smaller depth is farther.
fn transparent_fog(rgb: vec3<f32>, pixel: vec2<i32>, depth: f32, values: vec2<f32>, color: vec3<f32>) -> vec3<f32> {
    let sky = sky_texel(pixel).rgb;
    return improved_fog_mix(rgb, values, color, sky, cloud_at(pixel), cloud_depth_at(pixel) <= depth);
}

/// The cloud image over the image value `rgb` of the pixel at the reverse-Z `depth`.
fn with_clouds(rgb: vec3<f32>, pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    return cloud_over(rgb, cloud_at(pixel), cloud_depth_at(pixel), depth);
}

fn sky_with_clouds(pixel: vec2<i32>) -> vec3<f32> {
    return with_clouds(sky_texel(pixel).rgb, pixel, 0.0);
}
