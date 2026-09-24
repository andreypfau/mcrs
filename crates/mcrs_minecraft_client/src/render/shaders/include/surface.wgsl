#define_import_path mcrs_minecraft_client::surface

#import mcrs_minecraft_client::fields::{BIOME_TINTS, FIXED_TINTS}
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

/// 0 is untinted, the next few sample a biome colour map where the face stands, and the rest are
/// colours the block state fixes.
fn tint(index: u32, world_xz: vec2<f32>) -> vec3<f32> {
    if (index == 0u) {
        return vec3<f32>(1.0);
    }
    if (index <= BIOME_TINTS) {
        return textureSampleLevel(
            tints,
            tint_sampler,
            (world_xz - camera.tint_origin) * camera.tint_scale,
            index - 1u,
            0.0,
        ).rgb;
    }
    var fixed = FIXED_TINTS;
    let packed = fixed[index - 1u - BIOME_TINTS];
    return vec3<f32>(
        f32((packed >> 16u) & 0xffu),
        f32((packed >> 8u) & 0xffu),
        f32(packed & 0xffu),
    ) / 255.0;
}

fn shade_surface(s: Surface) -> vec4<f32> {
    let color = sprite_color(s.sprite, s.uv, s.ddx, s.ddy);
    let factor = tint(s.tint_kind, s.world_xz);
    return vec4<f32>(color.rgb * factor * s.shade, color.a);
}
