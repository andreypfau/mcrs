// Terrain rasterisation by vertex pulling: no vertex buffer, no index buffer, one draw per stream.
//
// Each instance is one quad. `instance_index` reads the compacted list the culling pass produced,
// which yields an index into the geometry arena; `vertex_index % 6` picks the corner. Greedy quads
// unpack from twelve bytes, baked model quads from four twelve-byte vertices.

#import bevy_render::view::View
#import bevy_render::globals::Globals

// The packed geometry layout. The mesher writes these words and declares the same names, and a
// test reads both sides back and fails if a width or an offset ever drifts apart.
fn field(word: u32, shift: u32, bits: u32) -> u32 {
    return (word >> shift) & ((1u << bits) - 1u);
}

/// Which word a field lives in is part of the layout, so it is read from the layout rather than
/// chosen at the call site. `base` is where the quad starts in the arena.
fn quad_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return field(quads[base + word], shift, bits);
}


const SECTION_SIZE: f32 = 16.0;

const LOCAL_X_WORD: u32 = 0u;
const LOCAL_X_SHIFT: u32 = 0u;
const LOCAL_X_BITS: u32 = 4u;
const LOCAL_Y_WORD: u32 = 0u;
const LOCAL_Y_SHIFT: u32 = 4u;
const LOCAL_Y_BITS: u32 = 3u;
const LOCAL_Z_WORD: u32 = 0u;
const LOCAL_Z_SHIFT: u32 = 7u;
const LOCAL_Z_BITS: u32 = 4u;

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
const QUAD_BLOCK_LIGHT_WORD: u32 = 0u;
const QUAD_BLOCK_LIGHT_SHIFT: u32 = 26u;
const QUAD_BLOCK_LIGHT_BITS: u32 = 4u;
const QUAD_TINT_WORD: u32 = 0u;
const QUAD_TINT_SHIFT: u32 = 30u;
const QUAD_TINT_BITS: u32 = 2u;
const QUAD_SKY_LIGHT_WORD: u32 = 2u;
const QUAD_SKY_LIGHT_SHIFT: u32 = 0u;
const QUAD_SKY_LIGHT_BITS: u32 = 4u;
/// How many words one greedy quad occupies.
const QUAD_WORDS: u32 = 3u;

const QUAD_AO_WORD: u32 = 1u;
const QUAD_AO_SHIFT: u32 = 0u;
const QUAD_AO_BITS: u32 = 8u;
const QUAD_FLIP_WORD: u32 = 1u;
const QUAD_FLIP_SHIFT: u32 = 8u;
const QUAD_FLIP_BITS: u32 = 1u;
const QUAD_SECTION_WORD: u32 = 1u;
const QUAD_SECTION_SHIFT: u32 = 9u;
const QUAD_SECTION_BITS: u32 = 11u;
const QUAD_ARRAY_WORD: u32 = 1u;
const QUAD_ARRAY_SHIFT: u32 = 20u;
const QUAD_ARRAY_BITS: u32 = 2u;
const QUAD_LAYER_WORD: u32 = 1u;
const QUAD_LAYER_SHIFT: u32 = 22u;
const QUAD_LAYER_BITS: u32 = 10u;

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

struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    // The corner of this draw's render region, in blocks. Explicit scalars rather than a vec3: a
    // vec3 would align to 16 and silently grow the struct.
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    cave_base: u32,
    wireframe: u32,
    overhang: f32,
    // The lowest layer number that names an animation rather than a layer of an array.
    animated_from: u32,
    // Where the biome colour map starts in world blocks and how far it reaches. The map covers the
    // loaded window, which does not start at the origin, so a world position has to be shifted and
    // scaled rather than divided by a constant.
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    pad1: u32,
}

/// The state of the sky the whole frame is lit by. Vanilla builds a sixteen by sixteen lightmap
/// texture out of these once a frame and samples it per vertex; there are few enough terms to
/// evaluate them where the sample would have been, which costs no texture and no upload.
struct Sky {
    /// `rgb` the colour sky light arrives in, `a` how much of it the time of day lets through.
    sky_light: vec4<f32>,
    /// `rgb` the tint of a torch at its dimmest, `a` the factor block light is scaled by.
    block_light: vec4<f32>,
    /// The floor under both, which is what keeps a sealed cave from being pure black.
    ambient: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<uniform> globals: Globals;
@group(0) @binding(3) var<uniform> sky: Sky;

fn model_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return field(vertices[base + word], shift, bits);
}

/// Where the section a quad names starts, in world blocks. A coordinate in a quad is relative to
/// its own section, and the region corner arrives with the draw, so this is the whole of what puts
/// the geometry back where it belongs.
fn section_origin(section: u32) -> vec3<f32> {
    let region = vec3<f32>(f32(params.origin_x), f32(params.origin_y), f32(params.origin_z));
    let local = vec3<f32>(
        f32(field(section, LOCAL_X_SHIFT, LOCAL_X_BITS)),
        f32(field(section, LOCAL_Y_SHIFT, LOCAL_Y_BITS)),
        f32(field(section, LOCAL_Z_SHIFT, LOCAL_Z_BITS)),
    );
    return region + local * SECTION_SIZE;
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

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(7) @interpolate(flat) array: u32,
    @location(2) shade: vec3<f32>,
    @location(3) world_xz: vec2<f32>,
    @location(4) @interpolate(flat) tint_kind: u32,
    /// Corner position within the quad, so the fragment can find the quad's own edges. Greedy
    /// quads cover many blocks, so this is not the same thing as the texture coordinate.
    @location(5) quad_uv: vec2<f32>,
    /// Signed distance to the diagonal the quad is split along, in quad-local units. Which of the
    /// two diagonals that is depends on the winding, so it is resolved here rather than in the
    /// fragment, which would otherwise need the winding flag too.
    @location(6) diagonal: f32,
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

/// Two triangles from four corners. The second winding is used when the ambient occlusion gradient
/// is anisotropic, so the split follows the darker diagonal instead of cutting across it.
fn corner_index(vertex: u32, flip: bool) -> u32 {
    if (flip) {
        switch vertex {
            case 0u: { return 1u; }
            case 1u: { return 2u; }
            case 2u: { return 3u; }
            case 3u: { return 1u; }
            case 4u: { return 3u; }
            default: { return 0u; }
        }
    }
    switch vertex {
        case 0u: { return 0u; }
        case 1u: { return 1u; }
        case 2u: { return 2u; }
        case 3u: { return 0u; }
        case 4u: { return 2u; }
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
) -> VertexOut {
    let quad = visible[params.visible_base + instance] * QUAD_WORDS;

    let anchor = section_origin(quad_field(quad, QUAD_SECTION_WORD, QUAD_SECTION_SHIFT, QUAD_SECTION_BITS))
        + vec3<f32>(
            f32(quad_field(quad, QUAD_X_WORD, QUAD_X_SHIFT, QUAD_X_BITS)),
            f32(quad_field(quad, QUAD_Y_WORD, QUAD_Y_SHIFT, QUAD_Y_BITS)),
            f32(quad_field(quad, QUAD_Z_WORD, QUAD_Z_SHIFT, QUAD_Z_BITS)),
        );
    let face = quad_field(quad, QUAD_FACE_WORD, QUAD_FACE_SHIFT, QUAD_FACE_BITS);
    let size = vec2<f32>(
        f32(quad_field(quad, QUAD_W_WORD, QUAD_W_SHIFT, QUAD_W_BITS) + 1u),
        f32(quad_field(quad, QUAD_H_WORD, QUAD_H_SHIFT, QUAD_H_BITS) + 1u),
    );
    let ao_bits = quad_field(quad, QUAD_AO_WORD, QUAD_AO_SHIFT, QUAD_AO_BITS);
    let block_light = f32(quad_field(quad, QUAD_BLOCK_LIGHT_WORD, QUAD_BLOCK_LIGHT_SHIFT, QUAD_BLOCK_LIGHT_BITS));
    let sky_light = f32(quad_field(quad, QUAD_SKY_LIGHT_WORD, QUAD_SKY_LIGHT_SHIFT, QUAD_SKY_LIGHT_BITS));
    let flip = quad_field(quad, QUAD_FLIP_WORD, QUAD_FLIP_SHIFT, QUAD_FLIP_BITS) == 1u;

    let corner = corner_index(vertex, flip);
    let quad_uv = corner_uv(corner);
    let c = quad_uv * size;
    let world = anchor + face_u_dir(face) * c.x + face_v_dir(face) * c.y;

    var out: VertexOut;
    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.uv = c;
    out.layer = quad_field(quad, QUAD_LAYER_WORD, QUAD_LAYER_SHIFT, QUAD_LAYER_BITS);
    out.array = quad_field(quad, QUAD_ARRAY_WORD, QUAD_ARRAY_SHIFT, QUAD_ARRAY_BITS);
    out.shade = lightmap(block_light, sky_light) * (face_shade(face) * ao_factor(ao_bits, corner));
    out.world_xz = world.xz;
    out.tint_kind = quad_field(quad, QUAD_TINT_WORD, QUAD_TINT_SHIFT, QUAD_TINT_BITS);
    out.quad_uv = quad_uv;
    out.diagonal = select(
        quad_uv.x - quad_uv.y,
        quad_uv.x + quad_uv.y - 1.0,
        flip,
    );
    return out;
}

@vertex
fn vertex_complex(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> VertexOut {
    let quad = visible[params.visible_base + instance];
    let corner = corner_index(vertex, false);
    let base = (quad * 4u + corner) * 3u;

    let local = vec3<f32>(
        f32(model_field(base, MODEL_X_WORD, MODEL_X_SHIFT, MODEL_X_BITS)),
        f32(model_field(base, MODEL_Y_WORD, MODEL_Y_SHIFT, MODEL_Y_BITS)),
        f32(model_field(base, MODEL_Z_WORD, MODEL_Z_SHIFT, MODEL_Z_BITS)),
    ) / MODEL_STEPS - MODEL_OVERHANG;
    let world = section_origin(model_field(base, MODEL_SECTION_WORD, MODEL_SECTION_SHIFT, MODEL_SECTION_BITS)) + local;
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

    var out: VertexOut;
    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(u, v);
    out.layer = model_field(base, MODEL_LAYER_WORD, MODEL_LAYER_SHIFT, MODEL_LAYER_BITS);
    out.array = model_field(base, MODEL_ARRAY_WORD, MODEL_ARRAY_SHIFT, MODEL_ARRAY_BITS);
    out.shade = lightmap(block_light, sky_light) * shade;
    out.world_xz = world.xz;
    out.tint_kind = model_field(base, MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS);
    let quad_uv = corner_uv(corner);
    out.quad_uv = quad_uv;
    out.diagonal = quad_uv.x - quad_uv.y;
    return out;
}

/// Distance from the fragment to the nearest edge of its own triangle, measured in pixels: the
/// screen-space derivative of a quad-local coordinate is how much of the quad one pixel spans, so
/// dividing by it keeps the outline a pixel wide whether the quad is one block across or a chunk
/// across. The split diagonal counts as an edge, so what is drawn is the triangle count.
fn edge_pixels(in: VertexOut) -> f32 {
    let width = max(fwidth(in.quad_uv), vec2<f32>(1e-6));
    let border = min(
        min(in.quad_uv.x, 1.0 - in.quad_uv.x) / width.x,
        min(in.quad_uv.y, 1.0 - in.quad_uv.y) / width.y,
    );
    let diagonal = abs(in.diagonal) / max(fwidth(in.diagonal), 1e-6);
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
/// else; above it, it names an animation instead, counting down from the top of the field. The
/// branch is coherent, since a quad is animated or it is not.
fn sprite_color(array: u32, uv: vec2<f32>, layer: u32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    if (layer < params.animated_from) {
        return sample_atlas(array, uv, layer, ddx, ddy);
    }
    let animation = animations[(1u << QUAD_LAYER_BITS) - 1u - layer];
    let elapsed = globals.time * TICKS_PER_SECOND / f32(animation.frametime);
    let step = u32(elapsed) % animation.count;
    let color = sample_atlas(array, uv, animation.base_layer + step, ddx, ddy);
    if (animation.interpolate == 0u) {
        return color;
    }
    let next = animation.base_layer + (step + 1u) % animation.count;
    return mix(color, sample_atlas(array, uv, next, ddx, ddy), fract(elapsed));
}

fn shade_sample(in: VertexOut) -> vec4<f32> {
    // Taken before the branch on whether the sprite animates, because a derivative is only defined
    // where control flow is uniform and which sprite a quad names is not.
    let ddx = dpdx(in.uv);
    let ddy = dpdy(in.uv);
    let color = sprite_color(in.array, in.uv, in.layer, ddx, ddy);
    // Both samples are unconditional: `tint_kind` varies between instances inside one draw, so a
    // branch around a `textureSample` would not be uniform control flow and WGSL rejects it.
    let tint_origin = vec2<f32>(f32(params.tint_origin_x), f32(params.tint_origin_z));
    let tint = textureSample(
        tints,
        tint_sampler,
        (in.world_xz - tint_origin) / vec2<f32>(params.tint_span_x, params.tint_span_z),
        max(in.tint_kind, 1u) - 1u,
    );
    let factor = select(vec3<f32>(1.0), tint.rgb, in.tint_kind != 0u);
    return vec4<f32>(color.rgb * factor * in.shade, color.a);
}

/// Keeping only the outline leaves the quad's interior unwritten, so the depth buffer stays open
/// there and the geometry behind shows through, all the way to the sky.
fn wireframe_discards(in: VertexOut) -> bool {
    // Taken before any `discard`: that demotes the invocation to a helper, and the neighbours a
    // derivative reads may be exactly the ones that were discarded.
    return params.wireframe != 0u && edge_pixels(in) > 1.0;
}

@fragment
fn fragment_opaque(in: VertexOut) -> @location(0) vec4<f32> {
    let color = shade_sample(in);
    let hidden = wireframe_discards(in);
    if (color.a < 0.5 || hidden) {
        discard;
    }
    return vec4<f32>(color.rgb, 1.0);
}

@fragment
fn fragment_blend(in: VertexOut) -> @location(0) vec4<f32> {
    let color = shade_sample(in);
    if (wireframe_discards(in)) {
        discard;
    }
    return color;
}
