// Terrain rasterisation by vertex pulling: no vertex buffer, no index buffer, one draw per stream.
//
// Each instance is one quad. `instance_index` reads the compacted list the culling pass produced,
// which yields an index into the geometry arena; `vertex_index` picks one of the four corners
// of a triangle strip. Greedy quads
// unpack from two words and read what they look like out of a side buffer, one entry per block
// face, because a greedy quad now covers many blocks of many kinds. Baked model quads still carry
// their appearance per vertex, four twelve-byte vertices to a quad.

#import bevy_render::view::View
#import bevy_render::globals::Globals
#import anvil_region_viewer::layout::{
    CULLED, Params, Sky, degenerate, section_origin,
}

// The packed geometry layout. The mesher writes these words and declares the same names, and a
// test reads both sides back and fails if a width or an offset ever drifts apart.

/// Which word a field lives in is part of the layout, so it is read from the layout rather than
/// chosen at the call site. `base` is where the quad starts in the arena.
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
/// How far below its own plane a fluid quad's top edge sits, in `MODEL_STEPS` per block. Zero on
/// everything else, so the whole block path runs through the same arithmetic untouched.
const QUAD_DROP_WORD: u32 = 0u;
const QUAD_DROP_SHIFT: u32 = 26u;
const QUAD_DROP_BITS: u32 = 5u;
/// Set on every quad of a fluid, which is held `FLUID_INSET` inside its own block.
const QUAD_FLUID_WORD: u32 = 0u;
const QUAD_FLUID_SHIFT: u32 = 31u;
const QUAD_FLUID_BITS: u32 = 1u;
/// How many words one greedy quad occupies.
const QUAD_WORDS: u32 = 2u;

const QUAD_SECTION_WORD: u32 = 1u;
const QUAD_SECTION_SHIFT: u32 = 0u;
const QUAD_SECTION_BITS: u32 = 11u;
const QUAD_FACE_BASE_WORD: u32 = 1u;
const QUAD_FACE_BASE_SHIFT: u32 = 11u;
const QUAD_FACE_BASE_BITS: u32 = 16u;

// One block face, as it looks. A greedy quad covers many blocks and names only where its run of
// these starts, so this is what the fragment reads instead of anything interpolated.
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
/// Set on the vertical faces of a fluid, which map their sprite from its top-left quarter rather
/// than one sprite to a block.
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

/// Model positions are fixed point in units of this many steps per block, offset by however far a
/// model may hang outside its own block so the result stays non-negative.
const MODEL_STEPS: f32 = 32.0;
const MODEL_OVERHANG: f32 = 2.0;

/// How far inside its own block every face of a fluid is held. A fluid face lands exactly where the
/// face of the block beside it lands — water against glass, against leaves, against the flat side
/// of a slab — and two quads in one plane have no order to be drawn in. Moving one of them back a
/// thousandth of a block gives them one, which is what vanilla does and for the same reason.
const FLUID_INSET: f32 = 0.001;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<uniform> globals: Globals;
@group(0) @binding(3) var<uniform> sky: Sky;

fn model_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return extractBits(vertices[base + word], shift, bits);
}

/// The corner of this draw's render region, which is what a section coordinate is relative to.
fn region_origin() -> vec3<f32> {
    return vec3<f32>(f32(params.origin_x), f32(params.origin_y), f32(params.origin_z));
}

@group(1) @binding(0) var<storage, read> quads: array<u32>;
@group(1) @binding(1) var<storage, read> vertices: array<u32>;
@group(1) @binding(2) var<storage, read> visible: array<u32>;
// One binding per sprite resolution rather than one atlas for everything: every layer of an array
// is the same size, so mixing resolutions in one would drag them all up to the largest.
@group(1) @binding(3) var atlas0: texture_2d_array<f32>;
@group(1) @binding(4) var atlas1: texture_2d_array<f32>;
@group(1) @binding(5) var atlas2: texture_2d_array<f32>;
@group(1) @binding(6) var atlas3: texture_2d_array<f32>;
@group(1) @binding(7) var atlas_sampler: sampler;
@group(1) @binding(8) var tints: texture_2d_array<f32>;
@group(1) @binding(9) var tint_sampler: sampler;
@group(1) @binding(10) var<storage, read> animations: array<Animation>;
// The head of each render region's block is a table of where its sections' faces start, indexed by
// the section number a quad carries; the attributes themselves follow it. A quad names a place
// inside its own section, which is what keeps it down to two words.
@group(1) @binding(11) var<storage, read> faces: array<u32>;

/// One animated sprite. Its sequence was laid out one step to a layer at load, so the step showing
/// now is arithmetic on the clock: there is no schedule to read and nothing to write per frame.
struct Animation {
    base_layer: u32,
    count: u32,
    /// Ticks one step lasts.
    frametime: u32,
    /// Whether a step blends into the next one instead of cutting to it.
    interpolate: u32,
};

/// Ticks a second, which is what a frame time in the metadata is counted in.
const TICKS_PER_SECOND: f32 = 20.0;

struct GreedyOut {
    @builtin(position) clip_position: vec4<f32>,
    /// Corner position within the quad, so the fragment can find both the block it landed on and
    /// the quad's own edges. The texture coordinate is this times the span and the diagonal the
    /// quad splits along is the difference of its two parts, so neither is carried: a corner
    /// writes what it hands the fragment stage to memory, and that is what the stage costs.
    @location(0) quad_uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    /// Where this quad's run of face attributes starts. The sprite, tint, light and ambient
    /// occlusion all come from there, one entry per block face, so none of them is carried either.
    @location(2) @interpolate(flat) face_base: u32,
    /// How many blocks the quad spans on each of its own axes. Also the stride of that run.
    @location(3) @interpolate(flat) face_span: vec2<u32>,
    /// Vanilla's directional shade for the face the quad points at.
    @location(4) @interpolate(flat) directional: f32,
};

/// A baked model quad carries its appearance per corner, so it needs neither the side buffer nor
/// anything that addresses it.
struct ModelOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) shade: vec3<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) array: u32,
    @location(5) @interpolate(flat) tint_kind: u32,
    /// Only the outline needs this, but the outline needs it from both kinds of geometry.
    @location(6) quad_uv: vec2<f32>,
};

/// What the two kinds of geometry agree on once each has worked out what its fragment looks like.
struct Surface {
    layer: u32,
    array: u32,
    tint_kind: u32,
    shade: vec3<f32>,
    uv: vec2<f32>,
    world_xz: vec2<f32>,
    /// How far the sprite coordinate moves across one pixel. Carried rather than taken at the
    /// sampling site because a fluid tiles its sprite inside every block, and the derivative of a
    /// coordinate that wraps is a whole sprite wide on the seam — one row of pixels a block that
    /// would otherwise drop to the coarsest mip there is.
    ddx: vec2<f32>,
    ddy: vec2<f32>,
};

/// Quad corners in the order vanilla winds them, as (u, v).
fn corner_uv(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(0.0, 1.0); }
        case 2u: { return vec2<f32>(1.0, 1.0); }
        default: { return vec2<f32>(1.0, 0.0); }
    }
}

/// Two triangles from four corners. There is only one winding now that no shading gradient spans
/// a quad: ambient occlusion is interpolated inside each block face, so no choice of diagonal can
/// smear one block's shadow across a merged run.
fn corner_index(vertex: u32) -> u32 {
    // A strip of four, wound so that its two triangles are the same pair, split along the same
    // diagonal, as the six-corner list this replaced: 1,2,0 and then 0,2,3.
    switch vertex {
        case 0u: { return 1u; }
        case 1u: { return 2u; }
        case 2u: { return 0u; }
        default: { return 3u; }
    }
}

/// World direction the sprite's `u` runs along for each face, sign folded in.
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

/// The outward normal of a face, which is the direction a fluid quad is held back along. Derived
/// from the two sprite axes rather than tabled a third time: a sign flipped in either of those
/// would otherwise leave a stale normal behind, and that shows up as a fluid surface inset the
/// wrong way rather than as anything that fails to compile.
fn face_normal(u_dir: vec3<f32>, v_dir: vec3<f32>) -> vec3<f32> {
    return -cross(u_dir, v_dir);
}

/// Vanilla's directional face shade, from `CardinalLighting`.
fn face_shade(face: u32) -> f32 {
    switch face {
        case 0u: { return 0.5; }
        case 1u: { return 1.0; }
        case 2u, 3u: { return 0.8; }
        default: { return 0.6; }
    }
}

/// Vanilla's light curve: how bright one light level reads, before any colour is applied.
fn light_curve(level: f32) -> f32 {
    let f = level / 15.0;
    return f / (4.0 - 3.0 * f);
}

/// What `lightmap.fsh` writes into the light texture, evaluated for one pair of light levels.
/// Sky light and block light are summed rather than maxed, and only the sky half is scaled by the
/// time of day — which is the whole of why a torch stays lit through the night.
fn lightmap(block_level: f32, sky_level: f32) -> vec3<f32> {
    var color = sky.ambient.rgb;
    color += sky.sky_light.rgb * light_curve(sky_level) * sky.sky_light.a;
    // Block light warms as it dims: the tint is strongest in the middle of the range and washes
    // out to white at either end.
    let f = block_level / 15.0;
    let parabolic = (2.0 * f - 1.0) * (2.0 * f - 1.0);
    let tint = mix(sky.block_light.rgb, vec3<f32>(1.0), 0.9 * parabolic);
    color += tint * light_curve(block_level) * sky.block_light.a;
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
}

/// Ambient occlusion samples average four neighbours that are each 0.2 when solid and 1.0 when
/// open, so the four reachable values are evenly spaced from 0.4 to 1.0.
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

    // A fluid surface stops short of the top of its block. The upward face sinks whole; a vertical
    // face keeps its floor and pulls only its top edge down, and doing that through the quad
    // coordinate rather than the position leaves that coordinate measuring depth below the block
    // boundary — which is exactly what the flowing texture is mapped by.
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
    // Held back off its own block boundary, where the face of the block beside it is drawn.
    let fluid = quad_field(quad, QUAD_FLUID_WORD, QUAD_FLUID_SHIFT, QUAD_FLUID_BITS);
    world -= face_normal(u_dir, v_dir) * (FLUID_INSET * f32(fluid));

    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.quad_uv = quad_uv;
    out.world_xz = world.xz;
    // Resolved here rather than in the fragment: the table lookup is the same for every fragment
    // of a quad, so it costs one load an instance instead of one a pixel.
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
    // The same denominator the mesher scaled by: the widest value the field holds.
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

/// Distance from the fragment to the nearest edge of its own triangle, measured in pixels: the
/// screen-space derivative of a quad-local coordinate is how much of the quad one pixel spans, so
/// dividing by it keeps the outline a pixel wide whether the quad is one block across or a chunk
/// across. The split diagonal counts as an edge, so what is drawn is the triangle count.
fn edge_pixels(quad_uv: vec2<f32>) -> f32 {
    let width = max(fwidth(quad_uv), vec2<f32>(1e-6));
    let border = min(
        min(quad_uv.x, 1.0 - quad_uv.x) / width.x,
        min(quad_uv.y, 1.0 - quad_uv.y) / width.y,
    );
    // Signed distance to the diagonal the quad splits along, which is the difference of the two
    // parts of the corner coordinate and so needs nothing carried of its own.
    let split = quad_uv.x - quad_uv.y;
    let diagonal = abs(split) / max(fwidth(split), 1e-6);
    return min(border, diagonal);
}

/// Which array a quad samples varies between instances of one draw, and a `textureSample` under
/// control flow that is not uniform is rejected outright. Asking for an exact level of detail
/// lifts the sample out of the branch: `textureSampleGrad` needs no implicit derivative, so the
/// switch is free to be as non-uniform as the data is. The derivatives come from the caller for
/// the same reason — taking one is only defined where control flow is uniform.
fn sample_atlas(array: u32, uv: vec2<f32>, layer: u32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    switch array {
        case 1u: { return textureSampleGrad(atlas1, atlas_sampler, uv, layer, ddx, ddy); }
        case 2u: { return textureSampleGrad(atlas2, atlas_sampler, uv, layer, ddx, ddy); }
        case 3u: { return textureSampleGrad(atlas3, atlas_sampler, uv, layer, ddx, ddy); }
        default: { return textureSampleGrad(atlas0, atlas_sampler, uv, layer, ddx, ddy); }
    }
}

/// A layer number below `animated_from` is a layer of the array and costs one sample and nothing
/// else; above it, it names an animation instead, counting down from the top of the field. Which
/// side of it a fragment falls on now varies within a quad, which costs nothing extra: everything
/// under the branch samples with the gradients handed in, so none of it needs uniform control flow.
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
    // Most of the world is untinted, and asking for an explicit level of detail is what lets the
    // sample sit under a branch at all: `textureSample` takes an implicit derivative and so demands
    // uniform control flow, which `tint_kind` does not have. The map is a single mip, so naming
    // level zero is what the implicit lookup would have found anyway.
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

/// A greedy quad covers many blocks of many kinds, so what it looks like is read here rather than
/// carried: one entry of the side buffer per block face, found from the corner coordinate.
fn greedy_surface(in: GreedyOut) -> Surface {
    // One unit to a block, which is both how the sprite tiles across the quad and how a fragment
    // finds which of the quad's faces it landed on.
    let uv = in.quad_uv * vec2<f32>(in.face_span);
    // Which of the quad's blocks this fragment landed on, and where inside it. The whole part is
    // the face and the fraction is the position on it. The clamp is for the far edge, where the
    // coordinate reaches the span exactly.
    let cell = min(vec2<u32>(max(uv, vec2<f32>(0.0))), in.face_span - vec2<u32>(1u));
    let attr = faces[in.face_base + cell.y * in.face_span.x + cell.x];
    let block_light = f32(extractBits(attr, FACE_BLOCK_LIGHT_SHIFT, FACE_BLOCK_LIGHT_BITS));
    let sky_light = f32(extractBits(attr, FACE_SKY_LIGHT_SHIFT, FACE_SKY_LIGHT_BITS));
    let ao_bits = extractBits(attr, FACE_AO_SHIFT, FACE_AO_BITS);
    // Bilinear over the face's own four corners, in the order the mesher packed them: 0 and 3
    // along the bottom edge, 1 and 2 along the top.
    let f = uv - vec2<f32>(cell);
    let ao = mix(
        mix(ao_factor(ao_bits, 0u), ao_factor(ao_bits, 3u), f.x),
        mix(ao_factor(ao_bits, 1u), ao_factor(ao_bits, 2u), f.x),
        f.y,
    );

    // A fluid's vertical faces sample the flowing sprite from its top-left quarter, once per block,
    // which is what shows the texture at twice the size a block face would and makes a waterfall
    // read as falling. The mesher leaves the occlusion field fully open on those faces, so nothing
    // above has to know they are a fluid.
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

/// Keeping only the outline leaves the quad's interior unwritten, so the depth buffer stays open
/// there and the geometry behind shows through, all the way to the sky.
fn wireframe_discards(quad_uv: vec2<f32>) -> bool {
    // Taken before any `discard`: that demotes the invocation to a helper, and the neighbours a
    // derivative reads may be exactly the ones that were discarded.
    return params.wireframe != 0u && edge_pixels(quad_uv) > 1.0;
}

/// The three ways a shaded fragment is finished with. Which one a draw uses is a property of the
/// stream, not of which of the two geometry representations it was built from, so each is stated
/// once here and the six entry points below are the six pairings.

/// Alpha tested, and so opaque wherever it survives.
fn finish_opaque(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    if (color.a < 0.5 || wireframe_discards(quad_uv)) {
        discard;
    }
    return vec4<f32>(color.rgb, 1.0);
}

/// A tile-based GPU can only drop a hidden fragment before shading it if the shader cannot change
/// whether the fragment exists. `discard` can, so an alpha-tested entry point forfeits hidden
/// surface removal and every solid pixel behind another gets shaded anyway. Solid sprites never
/// fail the alpha test, so they are drawn through this tail instead, and the sky and the terrain
/// behind the near terrain stop being shaded at all. The wireframe toggle discards, so it is not
/// offered here.
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
