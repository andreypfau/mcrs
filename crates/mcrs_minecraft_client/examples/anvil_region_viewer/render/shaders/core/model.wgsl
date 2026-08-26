
#import anvil_region_viewer::fields::{
    MODEL_ARRAY_WORD, MODEL_ARRAY_SHIFT, MODEL_ARRAY_BITS,
    MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS,
    MODEL_LAYER_WORD, MODEL_LAYER_SHIFT, MODEL_LAYER_BITS,
    MODEL_OVERHANG,
    MODEL_SECTION_WORD, MODEL_SECTION_SHIFT, MODEL_SECTION_BITS,
    MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS,
    MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS,
    MODEL_STEPS,
    MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS,
    MODEL_U_WORD, MODEL_U_SHIFT, MODEL_U_BITS,
    MODEL_V_WORD, MODEL_V_SHIFT, MODEL_V_BITS,
    MODEL_X_WORD, MODEL_X_SHIFT, MODEL_X_BITS,
    MODEL_Y_WORD, MODEL_Y_SHIFT, MODEL_Y_BITS,
    MODEL_Z_WORD, MODEL_Z_SHIFT, MODEL_Z_BITS,
}
#import anvil_region_viewer::finish::{finish_cutout, finish_solid, finish_translucent}
#import anvil_region_viewer::frame::{params, region_origin, view}
#import anvil_region_viewer::lighting::lightmap
#import anvil_region_viewer::quad::{corner_index, corner_uv}
#import anvil_region_viewer::region::{CULLED, degenerate, section_origin}
#import anvil_region_viewer::surface::{Surface, shade_surface}
#import anvil_region_viewer::terrain_bindings::{model_field, visible}

const WORDS_PER_VERTEX: u32 = 3u;
const CORNERS_PER_QUAD: u32 = 4u;

struct ModelOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) shade: vec3<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) array: u32,
    @location(5) @interpolate(flat) tint_kind: u32,
    @location(6) quad_uv: vec2<f32>,
};

fn shade_bucket(bucket: u32) -> f32 {
    switch bucket {
        case 0u: { return 0.5; }
        case 1u: { return 0.6; }
        case 2u: { return 0.8; }
        default: { return 1.0; }
    }
}

@vertex
fn vertex_model(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> ModelOut {
    let quad = visible[params.visible_base + instance];
    var out: ModelOut;
    if (quad == CULLED) {
        out.clip_position = degenerate();
        return out;
    }
    let corner = corner_index(vertex);
    let base = (quad * CORNERS_PER_QUAD + corner) * WORDS_PER_VERTEX;

    // Positions are stored in steps of a block and biased so a model may lean into its
    // neighbours by the overhang the mesher allowed for.
    let local = vec3<f32>(
        f32(model_field(base, MODEL_X_WORD, MODEL_X_SHIFT, MODEL_X_BITS)),
        f32(model_field(base, MODEL_Y_WORD, MODEL_Y_SHIFT, MODEL_Y_BITS)),
        f32(model_field(base, MODEL_Z_WORD, MODEL_Z_SHIFT, MODEL_Z_BITS)),
    ) / MODEL_STEPS - MODEL_OVERHANG;
    let world = section_origin(
        model_field(base, MODEL_SECTION_WORD, MODEL_SECTION_SHIFT, MODEL_SECTION_BITS),
        region_origin(),
    ) + local;

    let uv_scale = f32((1u << MODEL_U_BITS) - 1u);
    let u = f32(model_field(base, MODEL_U_WORD, MODEL_U_SHIFT, MODEL_U_BITS)) / uv_scale;
    let v = f32(model_field(base, MODEL_V_WORD, MODEL_V_SHIFT, MODEL_V_BITS)) / uv_scale;
    let block_light =
        f32(model_field(base, MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS));
    let sky_light =
        f32(model_field(base, MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS));
    let shade =
        shade_bucket(model_field(base, MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS));

    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(u, v);
    out.layer = model_field(base, MODEL_LAYER_WORD, MODEL_LAYER_SHIFT, MODEL_LAYER_BITS);
    out.array = model_field(base, MODEL_ARRAY_WORD, MODEL_ARRAY_SHIFT, MODEL_ARRAY_BITS);
    out.shade = lightmap(block_light, sky_light) * shade;
    out.world_xz = world.xz;
    out.tint_kind = model_field(base, MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS);
    out.quad_uv = corner_uv(corner);
    return out;
}

fn model_surface(in: ModelOut) -> Surface {
    var s: Surface;
    s.layer = in.layer;
    s.array = in.array;
    s.tint_kind = in.tint_kind;
    s.shade = in.shade;
    s.uv = in.uv;
    s.world_xz = in.world_xz;
    s.ddx = dpdx(in.uv);
    s.ddy = dpdy(in.uv);
    return s;
}

@fragment
fn fragment_model_solid(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_solid(shade_surface(model_surface(in)));
}

@fragment
fn fragment_model_cutout(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_cutout(shade_surface(model_surface(in)), in.quad_uv);
}

@fragment
fn fragment_model_translucent(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_translucent(shade_surface(model_surface(in)), in.quad_uv);
}
