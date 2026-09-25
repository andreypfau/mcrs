
#import mcrs_minecraft_client::fields::{
    MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS,
    MODEL_OVERHANG,
    MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS,
    MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS,
    MODEL_SPRITE_WORD, MODEL_SPRITE_SHIFT, MODEL_SPRITE_BITS,
    MODEL_STEPS,
    MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS,
    MODEL_TINT_HIGH_WORD, MODEL_TINT_HIGH_SHIFT, MODEL_TINT_HIGH_BITS,
    MODEL_U_WORD, MODEL_U_SHIFT, MODEL_U_BITS,
    MODEL_V_WORD, MODEL_V_SHIFT, MODEL_V_BITS,
    MODEL_X_WORD, MODEL_X_SHIFT, MODEL_X_BITS,
    MODEL_Y_WORD, MODEL_Y_SHIFT, MODEL_Y_BITS,
    MODEL_Z_WORD, MODEL_Z_SHIFT, MODEL_Z_BITS,
}
#import mcrs_minecraft_client::finish::{finish_cutout, finish_solid, finish_translucent}
#import mcrs_minecraft_client::frame::{camera, params}
#import mcrs_minecraft_client::lighting::lightmap
#import mcrs_minecraft_client::quad::{corner_index, corner_uv, quad_of}
#import mcrs_minecraft_client::section::section_origin
#import mcrs_minecraft_client::surface::{Surface, shade_surface}
#import mcrs_minecraft_client::terrain_bindings::{model_field, sections, visible, visible_slot}

const WORDS_PER_VERTEX: u32 = 3u;
const CORNERS_PER_QUAD: u32 = 4u;

struct ModelOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) shade: vec3<f32>,
    @location(3) @interpolate(flat) sprite: u32,
    @location(4) @interpolate(flat) tint_kind: u32,
    @location(5) quad_uv: vec2<f32>,
};

@vertex
fn vertex_model(@builtin(vertex_index) vertex: u32) -> ModelOut {
    var out: ModelOut;
    let entry = visible[visible_slot(quad_of(vertex))];
    let corner = corner_index(vertex);
    let base = (entry.x * CORNERS_PER_QUAD + corner) * WORDS_PER_VERTEX;

    // Positions are stored in steps of a block and biased so a model may lean into its
    // neighbours by the overhang the mesher allowed for.
    let local = vec3<f32>(
        f32(model_field(base, MODEL_X_WORD, MODEL_X_SHIFT, MODEL_X_BITS)),
        f32(model_field(base, MODEL_Y_WORD, MODEL_Y_SHIFT, MODEL_Y_BITS)),
        f32(model_field(base, MODEL_Z_WORD, MODEL_Z_SHIFT, MODEL_Z_BITS)),
    ) / MODEL_STEPS - MODEL_OVERHANG;
    let desc = sections[entry.y];
    let world = section_origin(desc) + local * f32(desc.scale);

    let uv_scale = f32((1u << MODEL_U_BITS) - 1u);
    let u = f32(model_field(base, MODEL_U_WORD, MODEL_U_SHIFT, MODEL_U_BITS)) / uv_scale;
    let v = f32(model_field(base, MODEL_V_WORD, MODEL_V_SHIFT, MODEL_V_BITS)) / uv_scale;
    let block_light =
        f32(model_field(base, MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS));
    let sky_light =
        f32(model_field(base, MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS));
    let shade = f32(model_field(base, MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS)) / 255.0;

    out.clip_position = camera.clip_from_relative * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(u, v);
    out.sprite = model_field(base, MODEL_SPRITE_WORD, MODEL_SPRITE_SHIFT, MODEL_SPRITE_BITS);
    out.shade = lightmap(block_light, sky_light) * shade;
    out.world_xz = world.xz;
    out.tint_kind = model_field(base, MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS)
        | model_field(base, MODEL_TINT_HIGH_WORD, MODEL_TINT_HIGH_SHIFT, MODEL_TINT_HIGH_BITS)
            << MODEL_TINT_BITS;
    out.quad_uv = corner_uv(corner);
    return out;
}

fn model_surface(in: ModelOut) -> Surface {
    var s: Surface;
    s.sprite = in.sprite;
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
    return finish_solid(shade_surface(model_surface(in)), in.quad_uv);
}

@fragment
fn fragment_model_cutout(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_cutout(shade_surface(model_surface(in)), in.quad_uv);
}

@fragment
fn fragment_model_translucent(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_translucent(shade_surface(model_surface(in)), in.quad_uv);
}

#ifdef DEFERRED
struct ModelDeferredOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) block_light: f32,
    @location(3) @interpolate(flat) sprite: u32,
    @location(4) @interpolate(flat) tint_kind: u32,
    @location(5) quad_uv: vec2<f32>,
    @location(6) sky_light: f32,
    @location(7) shade: f32,
    @location(8) @interpolate(flat) normal: vec3<f32>,
};

// WGSL cannot call an entry point, so this repeats `vertex_model`, keeping the light levels and
// the shade apart instead of multiplying them through the lightmap.
@vertex
fn vertex_model_deferred(@builtin(vertex_index) vertex: u32) -> ModelDeferredOut {
    var out: ModelDeferredOut;
    let entry = visible[visible_slot(quad_of(vertex))];
    let corner = corner_index(vertex);
    let base = (entry.x * CORNERS_PER_QUAD + corner) * WORDS_PER_VERTEX;

    let local = vec3<f32>(
        f32(model_field(base, MODEL_X_WORD, MODEL_X_SHIFT, MODEL_X_BITS)),
        f32(model_field(base, MODEL_Y_WORD, MODEL_Y_SHIFT, MODEL_Y_BITS)),
        f32(model_field(base, MODEL_Z_WORD, MODEL_Z_SHIFT, MODEL_Z_BITS)),
    ) / MODEL_STEPS - MODEL_OVERHANG;
    let desc = sections[entry.y];
    let world = section_origin(desc) + local * f32(desc.scale);

    let uv_scale = f32((1u << MODEL_U_BITS) - 1u);
    let u = f32(model_field(base, MODEL_U_WORD, MODEL_U_SHIFT, MODEL_U_BITS)) / uv_scale;
    let v = f32(model_field(base, MODEL_V_WORD, MODEL_V_SHIFT, MODEL_V_BITS)) / uv_scale;
    let block_light =
        f32(model_field(base, MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS));
    let sky_light =
        f32(model_field(base, MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS));
    let shade = f32(model_field(base, MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS)) / 255.0;

    out.clip_position = camera.clip_from_relative * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(u, v);
    out.sprite = model_field(base, MODEL_SPRITE_WORD, MODEL_SPRITE_SHIFT, MODEL_SPRITE_BITS);
    out.block_light = block_light;
    out.sky_light = sky_light;
    out.shade = shade;
    out.normal = model_quad_normal(entry.x);
    out.world_xz = world.xz;
    out.tint_kind = model_field(base, MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS)
        | model_field(base, MODEL_TINT_HIGH_WORD, MODEL_TINT_HIGH_SHIFT, MODEL_TINT_HIGH_BITS)
            << MODEL_TINT_BITS;
    out.quad_uv = corner_uv(corner);
    return out;
}

fn model_corner(quad: u32, corner: u32) -> vec3<f32> {
    let base = (quad * CORNERS_PER_QUAD + corner) * WORDS_PER_VERTEX;
    return vec3<f32>(
        f32(model_field(base, MODEL_X_WORD, MODEL_X_SHIFT, MODEL_X_BITS)),
        f32(model_field(base, MODEL_Y_WORD, MODEL_Y_SHIFT, MODEL_Y_BITS)),
        f32(model_field(base, MODEL_Z_WORD, MODEL_Z_SHIFT, MODEL_Z_BITS)),
    ) / MODEL_STEPS - MODEL_OVERHANG;
}

/// Models carry no normal, and one taken from derivatives breaks along triangle edges, so it is
/// the flat normal of the quad, facing out of its counter-clockwise front.
fn model_quad_normal(quad: u32) -> vec3<f32> {
    let p0 = model_corner(quad, 0u);
    let p1 = model_corner(quad, 1u);
    let p2 = model_corner(quad, 2u);
    return normalize(cross(p2 - p1, p0 - p1));
}

fn model_albedo(in: ModelDeferredOut) -> vec4<f32> {
    var classic: ModelOut;
    classic.clip_position = in.clip_position;
    classic.uv = in.uv;
    classic.world_xz = in.world_xz;
    classic.sprite = in.sprite;
    classic.tint_kind = in.tint_kind;
    classic.quad_uv = in.quad_uv;
    var s = model_surface(classic);
    s.shade = vec3<f32>(1.0);
    return shade_surface(s);
}

fn model_gbuffer(in: ModelDeferredOut, albedo: vec3<f32>) -> mcrs_minecraft_client::deferred::GBuffer {
    return mcrs_minecraft_client::deferred::gbuffer(albedo, in.shade, in.normal, in.block_light, in.sky_light);
}

@fragment
fn fragment_model_solid_deferred(in: ModelDeferredOut) -> mcrs_minecraft_client::deferred::GBuffer {
    let wireframe_discards = mcrs_minecraft_client::finish::wireframe_discards(in.quad_uv);
    let color = model_albedo(in);
    if (wireframe_discards) {
        discard;
    }
    return model_gbuffer(in, color.rgb);
}

@fragment
fn fragment_model_cutout_deferred(in: ModelDeferredOut) -> mcrs_minecraft_client::deferred::GBuffer {
    let wireframe_discards = mcrs_minecraft_client::finish::wireframe_discards(in.quad_uv);
    let color = model_albedo(in);
    if (color.a < 0.5 || wireframe_discards) {
        discard;
    }
    return model_gbuffer(in, color.rgb);
}

@fragment
fn fragment_model_translucent_deferred(in: ModelDeferredOut) -> @location(0) vec4<f32> {
    let wireframe_discards = mcrs_minecraft_client::finish::wireframe_discards(in.quad_uv);
    let color = model_albedo(in);
    if (wireframe_discards) {
        discard;
    }
    let lit = mcrs_minecraft_client::deferred::shade(color.rgb, in.shade, in.block_light, in.sky_light);
    return vec4<f32>(mcrs_minecraft_client::finish::to_target(lit), color.a);
}
#endif
