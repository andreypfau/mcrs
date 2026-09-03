#define_import_path mcrs_minecraft_client::surface

#import mcrs_minecraft_client::fields::{FACE_LAYER_BITS}
#import mcrs_minecraft_client::frame::camera
#import mcrs_minecraft_client::terrain_bindings::{
    animations, atlas0, atlas1, atlas2, atlas3, atlas_sampler, tint_sampler, tints,
}

struct Surface {
    layer: u32,
    array: u32,
    tint_kind: u32,
    shade: vec3<f32>,
    uv: vec2<f32>,
    world_xz: vec2<f32>,
    ddx: vec2<f32>,
    ddy: vec2<f32>,
};

/// Sprites live in one of a few atlases, chosen by resolution, and WGSL has no array of
/// textures to index, so the choice is a branch.
fn sample_atlas(array: u32, uv: vec2<f32>, layer: u32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    switch array {
        case 1u: { return textureSampleGrad(atlas1, atlas_sampler, uv, layer, ddx, ddy); }
        case 2u: { return textureSampleGrad(atlas2, atlas_sampler, uv, layer, ddx, ddy); }
        case 3u: { return textureSampleGrad(atlas3, atlas_sampler, uv, layer, ddx, ddy); }
        default: { return textureSampleGrad(atlas0, atlas_sampler, uv, layer, ddx, ddy); }
    }
}

/// Animated sprites are numbered down from the top of the layer range, so a layer at or above
/// `animated_from` names an animation rather than a still frame.
fn sprite_color(array: u32, uv: vec2<f32>, layer: u32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    if (layer < camera.animated_from) {
        return sample_atlas(array, uv, layer, ddx, ddy);
    }
    let frame = animations[(1u << FACE_LAYER_BITS) - 1u - layer];
    let color = sample_atlas(array, uv, frame.layer, ddx, ddy);
    if (frame.blend == 0.0) {
        return color;
    }
    return mix(color, sample_atlas(array, uv, frame.next, ddx, ddy), frame.blend);
}

fn shade_surface(s: Surface) -> vec4<f32> {
    let color = sprite_color(s.array, s.uv, s.layer, s.ddx, s.ddy);
    var factor = vec3<f32>(1.0);
    if (s.tint_kind != 0u) {
        factor = textureSampleLevel(
            tints,
            tint_sampler,
            (s.world_xz - camera.tint_origin) * camera.tint_scale,
            s.tint_kind - 1u,
            0.0,
        ).rgb;
    }
    return vec4<f32>(color.rgb * factor * s.shade, color.a);
}
