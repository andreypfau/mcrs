#import bevy_render::view::View
#import bevy_render::globals::Globals
#import anvil_region_viewer::layout::{
    CULLED, Params, Sky, degenerate, section_origin,
}


fn quad_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return extractBits(quads[base + word], shift, bits);
}

const QUAD_X_WORD: u32 = 0u;
const QUAD_X_SHIFT: u32 = 0u;
const QUAD_X_BITS: u32 = 5u;
const QUAD_Y_WORD: u32 = 0u;
const QUAD_Y_SHIFT: u32 = 5u;
const QUAD_Y_BITS: u32 = 5u;
const QUAD_Z_WORD: u32 = 0u;
const QUAD_Z_SHIFT: u32 = 10u;
const QUAD_Z_BITS: u32 = 5u;
const QUAD_FACE_WORD: u32 = 0u;
const QUAD_FACE_SHIFT: u32 = 15u;
const QUAD_FACE_BITS: u32 = 3u;
const QUAD_W_WORD: u32 = 0u;
const QUAD_W_SHIFT: u32 = 18u;
const QUAD_W_BITS: u32 = 4u;
const QUAD_H_WORD: u32 = 0u;
const QUAD_H_SHIFT: u32 = 22u;
const QUAD_H_BITS: u32 = 4u;
const QUAD_DROP_WORD: u32 = 0u;
const QUAD_DROP_SHIFT: u32 = 26u;
const QUAD_DROP_BITS: u32 = 5u;
const QUAD_FLUID_WORD: u32 = 0u;
const QUAD_FLUID_SHIFT: u32 = 31u;
const QUAD_FLUID_BITS: u32 = 1u;
const QUAD_WORDS: u32 = 2u;

const QUAD_SECTION_WORD: u32 = 1u;
const QUAD_SECTION_SHIFT: u32 = 0u;
const QUAD_SECTION_BITS: u32 = 11u;
const QUAD_FACE_BASE_WORD: u32 = 1u;
const QUAD_FACE_BASE_SHIFT: u32 = 11u;
const QUAD_FACE_BASE_BITS: u32 = 16u;

const FACE_LAYER_WORD: u32 = 0u;
const FACE_LAYER_SHIFT: u32 = 0u;
const FACE_LAYER_BITS: u32 = 10u;
const FACE_ARRAY_WORD: u32 = 0u;
const FACE_ARRAY_SHIFT: u32 = 10u;
const FACE_ARRAY_BITS: u32 = 2u;
const FACE_TINT_WORD: u32 = 0u;
const FACE_TINT_SHIFT: u32 = 12u;
const FACE_TINT_BITS: u32 = 2u;
const FACE_BLOCK_LIGHT_WORD: u32 = 0u;
const FACE_BLOCK_LIGHT_SHIFT: u32 = 14u;
const FACE_BLOCK_LIGHT_BITS: u32 = 4u;
const FACE_SKY_LIGHT_WORD: u32 = 0u;
const FACE_SKY_LIGHT_SHIFT: u32 = 18u;
const FACE_SKY_LIGHT_BITS: u32 = 4u;
const FACE_AO_WORD: u32 = 0u;
const FACE_AO_SHIFT: u32 = 22u;
const FACE_AO_BITS: u32 = 8u;
const FACE_FLUID_WORD: u32 = 0u;
const FACE_FLUID_SHIFT: u32 = 30u;
const FACE_FLUID_BITS: u32 = 1u;

const MODEL_X_WORD: u32 = 0u;
const MODEL_X_SHIFT: u32 = 0u;
const MODEL_X_BITS: u32 = 10u;
const MODEL_Y_WORD: u32 = 0u;
const MODEL_Y_SHIFT: u32 = 10u;
const MODEL_Y_BITS: u32 = 10u;
const MODEL_Z_WORD: u32 = 0u;
const MODEL_Z_SHIFT: u32 = 20u;
const MODEL_Z_BITS: u32 = 10u;

const MODEL_U_WORD: u32 = 1u;
const MODEL_U_SHIFT: u32 = 0u;
const MODEL_U_BITS: u32 = 10u;
const MODEL_V_WORD: u32 = 1u;
const MODEL_V_SHIFT: u32 = 10u;
const MODEL_V_BITS: u32 = 10u;
const MODEL_TINT_WORD: u32 = 1u;
const MODEL_TINT_SHIFT: u32 = 20u;
const MODEL_TINT_BITS: u32 = 2u;
const MODEL_BLOCK_LIGHT_WORD: u32 = 1u;
const MODEL_BLOCK_LIGHT_SHIFT: u32 = 22u;
const MODEL_BLOCK_LIGHT_BITS: u32 = 4u;
const MODEL_SHADE_WORD: u32 = 1u;
const MODEL_SHADE_SHIFT: u32 = 26u;
const MODEL_SHADE_BITS: u32 = 2u;
const MODEL_SKY_LIGHT_WORD: u32 = 1u;
const MODEL_SKY_LIGHT_SHIFT: u32 = 28u;
const MODEL_SKY_LIGHT_BITS: u32 = 4u;

const MODEL_SECTION_WORD: u32 = 2u;
const MODEL_SECTION_SHIFT: u32 = 0u;
const MODEL_SECTION_BITS: u32 = 11u;
const MODEL_ARRAY_WORD: u32 = 2u;
const MODEL_ARRAY_SHIFT: u32 = 11u;
const MODEL_ARRAY_BITS: u32 = 2u;
const MODEL_LAYER_WORD: u32 = 2u;
const MODEL_LAYER_SHIFT: u32 = 13u;
const MODEL_LAYER_BITS: u32 = 10u;

const MODEL_STEPS: f32 = 32.0;
const MODEL_OVERHANG: f32 = 2.0;

const FLUID_INSET: f32 = 0.001;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<uniform> globals: Globals;
@group(0) @binding(3) var<uniform> sky: Sky;

fn model_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return extractBits(vertices[base + word], shift, bits);
}

fn region_origin() -> vec3<f32> {
    return vec3<f32>(f32(params.origin_x), f32(params.origin_y), f32(params.origin_z));
}

@group(1) @binding(0) var<storage, read> quads: array<u32>;
@group(1) @binding(1) var<storage, read> vertices: array<u32>;
@group(1) @binding(2) var<storage, read> visible: array<u32>;
@group(1) @binding(3) var atlas0: texture_2d_array<f32>;
@group(1) @binding(4) var atlas1: texture_2d_array<f32>;
@group(1) @binding(5) var atlas2: texture_2d_array<f32>;
@group(1) @binding(6) var atlas3: texture_2d_array<f32>;
@group(1) @binding(7) var atlas_sampler: sampler;
@group(1) @binding(8) var tints: texture_2d_array<f32>;
@group(1) @binding(9) var tint_sampler: sampler;
@group(1) @binding(10) var<storage, read> animations: array<Animation>;
@group(1) @binding(11) var<storage, read> faces: array<u32>;

struct Animation {
    base_layer: u32,
    count: u32,
    frametime: u32,
    interpolate: u32,
};

const TICKS_PER_SECOND: f32 = 20.0;

struct GreedyOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) quad_uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) @interpolate(flat) face_base: u32,
    @location(3) @interpolate(flat) face_span: vec2<u32>,
    @location(4) @interpolate(flat) directional: f32,
};

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

fn corner_uv(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(0.0, 1.0); }
        case 2u: { return vec2<f32>(1.0, 1.0); }
        default: { return vec2<f32>(1.0, 0.0); }
    }
}

fn corner_index(vertex: u32) -> u32 {
    switch vertex {
        case 0u: { return 1u; }
        case 1u: { return 2u; }
        case 2u: { return 0u; }
        default: { return 3u; }
    }
}

fn face_u_dir(face: u32) -> vec3<f32> {
    switch face {
        case 0u, 1u, 3u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 2u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, 0.0, -1.0); }
    }
}

fn face_v_dir(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 1u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, -1.0, 0.0); }
    }
}

fn face_normal(u_dir: vec3<f32>, v_dir: vec3<f32>) -> vec3<f32> {
    return -cross(u_dir, v_dir);
}

fn face_shade(face: u32) -> f32 {
    switch face {
        case 0u: { return 0.5; }
        case 1u: { return 1.0; }
        case 2u, 3u: { return 0.8; }
        default: { return 0.6; }
    }
}

fn light_curve(level: f32) -> f32 {
    let f = level / 15.0;
    return f / (4.0 - 3.0 * f);
}

fn lightmap(block_level: f32, sky_level: f32) -> vec3<f32> {
    var color = sky.ambient.rgb;
    color += sky.sky_light.rgb * light_curve(sky_level) * sky.sky_light.a;
    let f = block_level / 15.0;
    let parabolic = (2.0 * f - 1.0) * (2.0 * f - 1.0);
    let tint = mix(sky.block_light.rgb, vec3<f32>(1.0), 0.9 * parabolic);
    color += tint * light_curve(block_level) * sky.block_light.a;
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn ao_factor(bits: u32, corner: u32) -> f32 {
    return 0.4 + f32((bits >> (corner * 2u)) & 3u) * 0.2;
}

@vertex
fn vertex_simple(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> GreedyOut {
    let culled = visible[params.visible_base + instance];
    var out: GreedyOut;
    if (culled == CULLED) {
        out.clip_position = degenerate();
        return out;
    }
    let quad = culled * QUAD_WORDS;

    let section = quad_field(quad, QUAD_SECTION_WORD, QUAD_SECTION_SHIFT, QUAD_SECTION_BITS);
    let anchor = section_origin(section, region_origin())
        + vec3<f32>(
            f32(quad_field(quad, QUAD_X_WORD, QUAD_X_SHIFT, QUAD_X_BITS)),
            f32(quad_field(quad, QUAD_Y_WORD, QUAD_Y_SHIFT, QUAD_Y_BITS)),
            f32(quad_field(quad, QUAD_Z_WORD, QUAD_Z_SHIFT, QUAD_Z_BITS)),
        );
    let face = quad_field(quad, QUAD_FACE_WORD, QUAD_FACE_SHIFT, QUAD_FACE_BITS);
    let span = vec2<u32>(
        quad_field(quad, QUAD_W_WORD, QUAD_W_SHIFT, QUAD_W_BITS) + 1u,
        quad_field(quad, QUAD_H_WORD, QUAD_H_SHIFT, QUAD_H_BITS) + 1u,
    );
    let size = vec2<f32>(span);

    let drop = f32(quad_field(quad, QUAD_DROP_WORD, QUAD_DROP_SHIFT, QUAD_DROP_BITS)) / MODEL_STEPS;
    var quad_uv = corner_uv(corner_index(vertex));
    if (face >= 2u) {
        quad_uv.y = max(quad_uv.y, drop / size.y);
    }
    let c = quad_uv * size;
    let u_dir = face_u_dir(face);
    let v_dir = face_v_dir(face);
    var world = anchor + u_dir * c.x + v_dir * c.y;
    if (face == 1u) {
        world.y -= drop;
    }
    let fluid = quad_field(quad, QUAD_FLUID_WORD, QUAD_FLUID_SHIFT, QUAD_FLUID_BITS);
    world -= face_normal(u_dir, v_dir) * (FLUID_INSET * f32(fluid));

    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.quad_uv = quad_uv;
    out.world_xz = world.xz;
    out.face_base = params.face_origin
        + faces[params.face_origin + section]
        + quad_field(quad, QUAD_FACE_BASE_WORD, QUAD_FACE_BASE_SHIFT, QUAD_FACE_BASE_BITS);
    out.face_span = span;
    out.directional = face_shade(face);
    return out;
}

@vertex
fn vertex_complex(
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
    let base = (quad * 4u + corner) * 3u;

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
    let block_light = f32(model_field(base, MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS));
    let sky_light = f32(model_field(base, MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS));
    let shade_bucket = model_field(base, MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS);
    var shade = 1.0;
    switch shade_bucket {
        case 0u: { shade = 0.5; }
        case 1u: { shade = 0.6; }
        case 2u: { shade = 0.8; }
        default: { shade = 1.0; }
    }

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

fn edge_pixels(quad_uv: vec2<f32>) -> f32 {
    let width = max(fwidth(quad_uv), vec2<f32>(1e-6));
    let border = min(
        min(quad_uv.x, 1.0 - quad_uv.x) / width.x,
        min(quad_uv.y, 1.0 - quad_uv.y) / width.y,
    );
    let split = quad_uv.x - quad_uv.y;
    let diagonal = abs(split) / max(fwidth(split), 1e-6);
    return min(border, diagonal);
}

fn sample_atlas(array: u32, uv: vec2<f32>, layer: u32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    switch array {
        case 1u: { return textureSampleGrad(atlas1, atlas_sampler, uv, layer, ddx, ddy); }
        case 2u: { return textureSampleGrad(atlas2, atlas_sampler, uv, layer, ddx, ddy); }
        case 3u: { return textureSampleGrad(atlas3, atlas_sampler, uv, layer, ddx, ddy); }
        default: { return textureSampleGrad(atlas0, atlas_sampler, uv, layer, ddx, ddy); }
    }
}

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

fn greedy_surface(in: GreedyOut) -> Surface {
    let uv = in.quad_uv * vec2<f32>(in.face_span);
    let cell = min(vec2<u32>(max(uv, vec2<f32>(0.0))), in.face_span - vec2<u32>(1u));
    let attr = faces[in.face_base + cell.y * in.face_span.x + cell.x];
    let block_light = f32(extractBits(attr, FACE_BLOCK_LIGHT_SHIFT, FACE_BLOCK_LIGHT_BITS));
    let sky_light = f32(extractBits(attr, FACE_SKY_LIGHT_SHIFT, FACE_SKY_LIGHT_BITS));
    let ao_bits = extractBits(attr, FACE_AO_SHIFT, FACE_AO_BITS);
    let f = uv - vec2<f32>(cell);
    let ao = mix(
        mix(ao_factor(ao_bits, 0u), ao_factor(ao_bits, 3u), f.x),
        mix(ao_factor(ao_bits, 1u), ao_factor(ao_bits, 2u), f.x),
        f.y,
    );

    let fluid = extractBits(attr, FACE_FLUID_SHIFT, FACE_FLUID_BITS) != 0u;
    let scale = select(1.0, 0.5, fluid);

    var s: Surface;
    s.layer = extractBits(attr, FACE_LAYER_SHIFT, FACE_LAYER_BITS);
    s.array = extractBits(attr, FACE_ARRAY_SHIFT, FACE_ARRAY_BITS);
    s.tint_kind = extractBits(attr, FACE_TINT_SHIFT, FACE_TINT_BITS);
    s.shade = lightmap(block_light, sky_light) * (in.directional * ao);
    s.uv = select(uv, fract(uv) * 0.5, fluid);
    s.world_xz = in.world_xz;
    s.ddx = dpdx(uv) * scale;
    s.ddy = dpdy(uv) * scale;
    return s;
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

fn wireframe_discards(quad_uv: vec2<f32>) -> bool {
    return params.wireframe != 0u && edge_pixels(quad_uv) > 1.0;
}


fn finish_opaque(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    if (color.a < 0.5 || wireframe_discards(quad_uv)) {
        discard;
    }
    return vec4<f32>(color.rgb, 1.0);
}

fn finish_solid(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(color.rgb, 1.0);
}

fn finish_blend(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    if (wireframe_discards(quad_uv)) {
        discard;
    }
    return color;
}

@fragment
fn fragment_greedy_opaque(in: GreedyOut) -> @location(0) vec4<f32> {
    return finish_opaque(shade_surface(greedy_surface(in)), in.quad_uv);
}

@fragment
fn fragment_greedy_solid(in: GreedyOut) -> @location(0) vec4<f32> {
    return finish_solid(shade_surface(greedy_surface(in)));
}

@fragment
fn fragment_greedy_blend(in: GreedyOut) -> @location(0) vec4<f32> {
    return finish_blend(shade_surface(greedy_surface(in)), in.quad_uv);
}

@fragment
fn fragment_model_opaque(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_opaque(shade_surface(model_surface(in)), in.quad_uv);
}

@fragment
fn fragment_model_solid(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_solid(shade_surface(model_surface(in)));
}

@fragment
fn fragment_model_blend(in: ModelOut) -> @location(0) vec4<f32> {
    return finish_blend(shade_surface(model_surface(in)), in.quad_uv);
}
