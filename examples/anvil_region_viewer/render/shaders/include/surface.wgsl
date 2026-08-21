#define_import_path anvil_region_viewer::surface

#import anvil_region_viewer::fields::{FACE_LAYER_BITS}
#import anvil_region_viewer::frame::{globals, params}
#import anvil_region_viewer::terrain_bindings::{
    animations, atlas0, atlas1, atlas2, atlas3, atlas_sampler, tint_sampler, tints,
}

const TICKS_PER_SECOND: f32 = 20.0;

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
    if (layer < params.animated_from) {
        return sample_atlas(array, uv, layer, ddx, ddy);
    }
    let animation = animations[(1u << FACE_LAYER_BITS) - 1u - layer];
    let elapsed = globals.time * TICKS_PER_SECOND / f32(animation.frametime);
    let step = u32(elapsed) % animation.count;
    let color = sample_atlas(array, uv, animation.base_layer + step, ddx, ddy);
    if (animation.interpolate == 0u) {
        return color;
    }
    let next = animation.base_layer + (step + 1u) % animation.count;
    return mix(color, sample_atlas(array, uv, next, ddx, ddy), fract(elapsed));
}

fn shade_surface(s: Surface) -> vec4<f32> {
    let color = sprite_color(s.array, s.uv, s.layer, s.ddx, s.ddy);
    var factor = vec3<f32>(1.0);
    if (s.tint_kind != 0u) {
        let tint_origin = vec2<f32>(f32(params.tint_origin_x), f32(params.tint_origin_z));
        factor = textureSampleLevel(
            tints,
            tint_sampler,
            (s.world_xz - tint_origin) / vec2<f32>(params.tint_span_x, params.tint_span_z),
            s.tint_kind - 1u,
            0.0,
        ).rgb;
    }
    return vec4<f32>(color.rgb * factor * s.shade, color.a);
}
