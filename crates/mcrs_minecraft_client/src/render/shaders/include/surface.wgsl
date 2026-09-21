#define_import_path mcrs_minecraft_client::surface

#import mcrs_minecraft_client::frame::camera
#import mcrs_minecraft_client::terrain_bindings::{
    STILL, animations, atlas0, atlas1, atlas2, atlas3, atlas_sampler, sprites, tint_sampler,
    tints,
}

struct Surface {
    sprite: u32,
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

fn sprite_color(sprite: u32, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    let entry = sprites[sprite];
    let array = entry.array_layer >> 16u;
    if (entry.animation == STILL) {
        return sample_atlas(array, uv, entry.array_layer & 0xFFFFu, ddx, ddy);
    }
    let frame = animations[entry.animation];
    let color = sample_atlas(array, uv, frame.layer, ddx, ddy);
    if (frame.blend == 0.0) {
        return color;
    }
    return mix(color, sample_atlas(array, uv, frame.next, ddx, ddy), frame.blend);
}

fn shade_surface(s: Surface) -> vec4<f32> {
    let color = sprite_color(s.sprite, s.uv, s.ddx, s.ddy);
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
