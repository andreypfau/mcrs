import mcrs_minecraft_render::shaders::include::depth::depth_at;
import mcrs_minecraft_render::shaders::include::finish::from_target;
import mcrs_minecraft_render::shaders::include::fog::{cloud_over, improved_fog_mix};
import mcrs_minecraft_render::shaders::include::transparency_layout::{
    cloud_depth, cloud_image, cloud_nearest, sky_image,
};

fn sky_texel(pixel: vec2<i32>) -> vec4<f32> {
    return textureLoad(sky_image, pixel, 0);
}

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

fn transparent_fog(rgb: vec3<f32>, pixel: vec2<i32>, depth: f32, values: vec2<f32>, color: vec3<f32>) -> vec3<f32> {
    let sky = sky_texel(pixel).rgb;
    return improved_fog_mix(rgb, values, color, sky, cloud_at(pixel), cloud_depth_at(pixel) <= depth);
}

fn with_clouds(rgb: vec3<f32>, pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    return cloud_over(rgb, cloud_at(pixel), cloud_depth_at(pixel), depth);
}

fn sky_with_clouds(pixel: vec2<i32>) -> vec3<f32> {
    return with_clouds(sky_texel(pixel).rgb, pixel, 0.0);
}
