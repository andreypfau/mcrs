// The records of the entity passes. `records.rs` and `tables.rs` lay out the same bytes; change
// both together. Each record uses scalar fields, so no WGSL alignment rule moves a field.

struct Frame {
    clip_from_relative: mat4x4<f32>,
    frustum: array<vec4<f32>, 5>,
    camera_section: vec3<i32>,
    tick: u32,
    camera_offset: vec3<f32>,
    partial_tick: f32,
    viewport: vec2<f32>,
    projection_scale: f32,
    height_threshold: f32,
    opaque_zero: u32,
    slot_count: u32,
    hiz_levels: u32,
    flags: u32,
    scatter_count: u32,
    bone_capacity: u32,
    visible_capacity: u32,
    face_capacity: u32,
}

const FRAME_NETHER_LIGHT: u32 = 1u;

struct TickRecord {
    bx: i32,
    by: i32,
    bz: i32,
    fraction_xy: u32,
    fraction_z_body: u32,
    head_pitch: u32,
    walk_position: f32,
    walk_speed: f32,
}

struct CullRecord {
    bx: i32,
    by: i32,
    bz: i32,
    radius_flags: u32,
}

const CULL_VALID: u32 = 1u;
const CULL_FULL_RIG_ONLY: u32 = 2u;

struct StateRecord {
    definition_mask: u32,
    layer0: u32,
    layer1: u32,
    layer2: u32,
    layer3: u32,
    flags: u32,
    hurt_start: f32,
    death_start: f32,
    light_scale: u32,
    section: u32,
    clock_offset: i32,
    input: u32,
}

const STATE_BABY: u32 = 1u;
const STATE_INVISIBLE: u32 = 2u;
const STATE_UPSIDE_DOWN: u32 = 4u;
const STATE_CLOCK_STOPPED: u32 = 8u;
const STATE_DYING: u32 = 16u;
const STATE_HIDDEN_SHIFT: u32 = 16u;

struct InputRecord {
    values: array<f32, 16>,
    clips: array<vec2<u32>, 4>,
}

struct ModelRecord {
    bound_x: f32,
    bound_y: f32,
    bound_z: f32,
    bound_radius: f32,
    visible_min_x: f32,
    visible_min_y: f32,
    visible_min_z: f32,
    bone_count: u32,
    visible_max_x: f32,
    visible_max_y: f32,
    visible_max_z: f32,
    first_bone: u32,
    texture_width: f32,
    texture_height: f32,
    first_cuboid: u32,
    cuboid_count: u32,
}

struct BoneRecord {
    pivot_x: f32,
    pivot_y: f32,
    pivot_z: f32,
    parent_palette: u32,
    rest_x: f32,
    rest_y: f32,
    rest_z: f32,
    flags: u32,
    scale_x: f32,
    scale_y: f32,
    scale_z: f32,
    spare: u32,
}

const NO_PARENT: u32 = 0xffffu;
const NO_PALETTE: u32 = 0xffffu;
const BONE_NEVER_RENDER: u32 = 1u;

struct CuboidRecord {
    origin_x: f32,
    origin_y: f32,
    origin_z: f32,
    size_x: f32,
    size_y: f32,
    size_z: f32,
    uv: u32,
    inflate_bone_flags: u32,
}

const CUBOID_MIRROR: u32 = 64u;
const CUBOID_EXPLICIT: u32 = 128u;
const INFLATE_STEP: f32 = 1.0 / 8192.0;

struct Variant {
    array_layer: u32,
    uv_scale_x: f32,
    uv_scale_y: f32,
    cutout_first: u32,
}

/// The entity transform of a visible mob, from internal space in model units to the space
/// relative to the origin of the camera section, as three rows; then its overlay and its light.
struct Instance {
    row0: vec4<f32>,
    row1: vec4<f32>,
    row2: vec4<f32>,
    overlay: vec4<f32>,
    light: vec4<f32>,
}

const FACE_EAST: u32 = 0u;
const FACE_NORTH: u32 = 1u;
const FACE_WEST: u32 = 2u;
const FACE_SOUTH: u32 = 3u;
const FACE_UP: u32 = 4u;
const FACE_DOWN: u32 = 5u;

fn layer_word(state: StateRecord, layer: u32) -> u32 {
    switch layer {
        case 0u: { return state.layer0; }
        case 1u: { return state.layer1; }
        case 2u: { return state.layer2; }
        default: { return state.layer3; }
    }
}

fn layer_mask(state: StateRecord) -> u32 {
    return (state.definition_mask >> 16u) & 0xffu;
}

fn state_definition(state: StateRecord) -> u32 {
    return state.definition_mask & 0xffffu;
}

fn face_normal(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 1u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 2u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 3u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 4u: { return vec3<f32>(0.0, 1.0, 0.0); }
        default: { return vec3<f32>(0.0, -1.0, 0.0); }
    }
}

/// The drawn box of a cuboid: the box grown by the inflate, collapsed to its middle on an axis
/// where a negative inflate turns it inside out.
struct DrawnBox {
    lo: vec3<f32>,
    hi: vec3<f32>,
}

fn cuboid_inflate(cuboid: CuboidRecord) -> f32 {
    let bits = cuboid.inflate_bone_flags & 0xffffu;
    return f32(bitcast<i32>(bits << 16u) >> 16u) * INFLATE_STEP;
}

fn cuboid_bone(cuboid: CuboidRecord) -> u32 {
    return (cuboid.inflate_bone_flags >> 16u) & 0xffu;
}

fn cuboid_flags(cuboid: CuboidRecord) -> u32 {
    return cuboid.inflate_bone_flags >> 24u;
}

fn cuboid_size(cuboid: CuboidRecord) -> vec3<f32> {
    return vec3<f32>(cuboid.size_x, cuboid.size_y, cuboid.size_z);
}

fn drawn_box(cuboid: CuboidRecord) -> DrawnBox {
    let lo = vec3<f32>(cuboid.origin_x, cuboid.origin_y, cuboid.origin_z);
    let hi = lo + cuboid_size(cuboid);
    let inflate = cuboid_inflate(cuboid);
    let grown_lo = lo - vec3<f32>(inflate);
    let grown_hi = hi + vec3<f32>(inflate);
    let flat = grown_lo > grown_hi;
    let middle = (lo + hi) * 0.5;
    return DrawnBox(select(grown_lo, middle, flat), select(grown_hi, middle, flat));
}

/// The corners `c0`, `c1`, `c2` and `c3` of a face of the box from `lo` to `hi`.
fn face_corner(face: u32, corner: u32, lo: vec3<f32>, hi: vec3<f32>) -> vec3<f32> {
    let right = (corner & 1u) != 0u;
    let low = (corner & 2u) != 0u;
    switch face {
        case 0u: { return vec3<f32>(hi.x, select(hi.y, lo.y, low), select(hi.z, lo.z, right)); }
        case 2u: { return vec3<f32>(lo.x, select(hi.y, lo.y, low), select(lo.z, hi.z, right)); }
        case 4u: { return vec3<f32>(select(lo.x, hi.x, right), hi.y, select(lo.z, hi.z, low)); }
        case 5u: { return vec3<f32>(select(lo.x, hi.x, right), lo.y, select(hi.z, lo.z, low)); }
        case 3u: { return vec3<f32>(select(lo.x, hi.x, right), select(hi.y, lo.y, low), hi.z); }
        default: { return vec3<f32>(select(hi.x, lo.x, right), select(hi.y, lo.y, low), lo.z); }
    }
}

/// The texel of a corner of a face of a box UV with the origin `uv`, the layout sizes
/// `layout` (`w`, `h`, `d`) and the mirror flag.
fn box_uv_corner(face: u32, corner: u32, uv: vec2<f32>, sizes: vec3<f32>, mirror: bool) -> vec2<f32> {
    let u = uv.x;
    let v = uv.y;
    let w = sizes.x;
    let h = sizes.y;
    let d = sizes.z;
    var laid_out = face;
    if (mirror && face == FACE_EAST) {
        laid_out = FACE_WEST;
    } else if (mirror && face == FACE_WEST) {
        laid_out = FACE_EAST;
    }
    var rect: vec4<f32>;
    switch laid_out {
        case 0u: { rect = vec4<f32>(u, v + d, u + d, v + d + h); }
        case 1u: { rect = vec4<f32>(u + d, v + d, u + d + w, v + d + h); }
        case 2u: { rect = vec4<f32>(u + d + w, v + d, u + 2.0 * d + w, v + d + h); }
        case 3u: { rect = vec4<f32>(u + 2.0 * d + w, v + d, u + 2.0 * d + 2.0 * w, v + d + h); }
        case 4u: { rect = vec4<f32>(u + d + w, v + d, u + d, v); }
        default: { rect = vec4<f32>(u + d + 2.0 * w, v, u + d + w, v + d); }
    }
    if (mirror) {
        rect = vec4<f32>(rect.z, rect.y, rect.x, rect.w);
    }
    let right = (corner & 1u) != 0u;
    let low = (corner & 2u) != 0u;
    return vec2<f32>(select(rect.x, rect.z, right), select(rect.y, rect.w, low));
}

fn cuboid_uv(cuboid: CuboidRecord) -> vec2<f32> {
    let u = bitcast<i32>(cuboid.uv << 16u) >> 16u;
    let v = bitcast<i32>(cuboid.uv) >> 16u;
    return vec2<f32>(f32(u), f32(v));
}

fn transform_point(row0: vec4<f32>, row1: vec4<f32>, row2: vec4<f32>, p: vec3<f32>) -> vec3<f32> {
    let h = vec4<f32>(p, 1.0);
    return vec3<f32>(dot(row0, h), dot(row1, h), dot(row2, h));
}

fn transform_vector(row0: vec4<f32>, row1: vec4<f32>, row2: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(row0.xyz, v), dot(row1.xyz, v), dot(row2.xyz, v));
}

/// The direction of a normal under the linear part of a transform with these rows: the
/// inverse transpose, with the sign of the determinant, so a mirror keeps it outward.
fn transform_normal(row0: vec4<f32>, row1: vec4<f32>, row2: vec4<f32>, n: vec3<f32>) -> vec3<f32> {
    let a = vec3<f32>(row0.x, row1.x, row2.x);
    let b = vec3<f32>(row0.y, row1.y, row2.y);
    let c = vec3<f32>(row0.z, row1.z, row2.z);
    let bc = cross(b, c);
    let cofactor = bc * n.x + cross(c, a) * n.y + cross(a, b) * n.z;
    let det = dot(a, bc);
    return normalize(select(cofactor, -cofactor, det < 0.0));
}

/// A transform as three rows.
struct Placed {
    row0: vec4<f32>,
    row1: vec4<f32>,
    row2: vec4<f32>,
}

fn compose_row(r: vec4<f32>, b0: vec4<f32>, b1: vec4<f32>, b2: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        r.x * b0.x + r.y * b1.x + r.z * b2.x,
        r.x * b0.y + r.y * b1.y + r.z * b2.y,
        r.x * b0.z + r.y * b1.z + r.z * b2.z,
        r.x * b0.w + r.y * b1.w + r.z * b2.w + r.w,
    );
}

/// The instance transform after the bone transform with the rows `b0`, `b1` and `b2`.
fn compose_rows(instance: Instance, b0: vec4<f32>, b1: vec4<f32>, b2: vec4<f32>) -> Placed {
    return Placed(
        compose_row(instance.row0, b0, b1, b2),
        compose_row(instance.row1, b0, b1, b2),
        compose_row(instance.row2, b0, b1, b2),
    );
}
