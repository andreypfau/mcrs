// Frustum and backface-group culling for one geometry stream.
//
// One workgroup handles one group — a run of quads from a single section and face group. Thread 0
// decides whether the whole run survives and reserves a contiguous span in the visible list with a
// single atomic; the other 63 threads then scatter the quad indices into it. That keeps the atomic
// traffic at one operation per run rather than one per quad, and the draw that follows is a single
// `draw_indirect` whose instance count the same atomic produced.

#import bevy_render::view::View

// The packed geometry layout. The mesher writes these words and declares the same names, and a
// test reads both sides back and fails if a width or an offset ever drifts apart.
fn field(word: u32, shift: u32, bits: u32) -> u32 {
    return (word >> shift) & ((1u << bits) - 1u);
}

const SECTION_X_SHIFT: u32 = 0u;
const SECTION_X_BITS: u32 = 5u;
const SECTION_Y_SHIFT: u32 = 5u;
const SECTION_Y_BITS: u32 = 5u;
const SECTION_Z_SHIFT: u32 = 10u;
const SECTION_Z_BITS: u32 = 5u;
const GROUP_FACE_SHIFT: u32 = 15u;
const GROUP_FACE_BITS: u32 = 3u;
const SECTION_INDEX_SHIFT: u32 = 0u;
const SECTION_INDEX_BITS: u32 = 15u;

struct Group {
    quad_base: u32,
    quad_count: u32,
    // The section this run came from, in units of 16 blocks, and the face group its quads point at.
    section: u32,
    reserved: u32,
}

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    min_section_y: i32,
    wireframe: u32,
    // How far this stream's geometry may reach outside its own section. Explicit scalars rather
    // than a vec3: a vec3 would align to 16 and silently grow the struct past the 32 bytes the
    // dynamic uniform offsets are laid out on.
    overhang: f32,
    pad1: u32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(1) @binding(0) var<storage, read> groups: array<Group>;
@group(1) @binding(1) var<storage, read_write> visible: array<u32>;
@group(1) @binding(2) var<storage, read_write> args: array<DrawArgs>;
@group(1) @binding(3) var<storage, read> cave_visible: array<u32>;

const CULLED: u32 = 0xffffffffu;

/// Blocks along one edge of a section.
const SECTION_SIZE: i32 = 16;

var<workgroup> reserved_slot: u32;

fn section_min(g: Group) -> vec3<f32> {
    let sx = i32(field(g.section, SECTION_X_SHIFT, SECTION_X_BITS));
    let sy = i32(field(g.section, SECTION_Y_SHIFT, SECTION_Y_BITS)) + params.min_section_y;
    let sz = i32(field(g.section, SECTION_Z_SHIFT, SECTION_Z_BITS));
    return vec3<f32>(f32(sx * SECTION_SIZE), f32(sy * SECTION_SIZE), f32(sz * SECTION_SIZE));
}

/// The half spaces in `view.frustum` contain a point when `dot(plane.xyz, p) + plane.w > 0`, so the
/// box is outside as soon as its most positive corner along a plane normal falls behind that plane.
fn in_frustum(mn: vec3<f32>, mx: vec3<f32>) -> bool {
    for (var i = 0u; i < 6u; i = i + 1u) {
        let plane = view.frustum[i];
        let corner = vec3<f32>(
            select(mn.x, mx.x, plane.x > 0.0),
            select(mn.y, mx.y, plane.y > 0.0),
            select(mn.z, mx.z, plane.z > 0.0),
        );
        if (dot(plane.xyz, corner) + plane.w <= 0.0) {
            return false;
        }
    }
    return true;
}

/// Every quad in a face group shares one outward normal, so a group facing away from the camera is
/// entirely backfacing. At least three of the six groups fail this for any camera position.
fn faces_camera(face: u32, mn: vec3<f32>, mx: vec3<f32>) -> bool {
    let cam = view.world_position;
    switch face {
        case 0u: { return cam.y < mx.y; }
        case 1u: { return cam.y > mn.y; }
        case 2u: { return cam.z < mx.z; }
        case 3u: { return cam.z > mn.z; }
        case 4u: { return cam.x < mx.x; }
        case 5u: { return cam.x > mn.x; }
        default: { return true; }
    }
}

@compute @workgroup_size(64)
fn cull(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    if (workgroup.x >= params.group_count) {
        return;
    }
    let g = groups[params.group_base + workgroup.x];

    if (local == 0u) {
        // The face group has to be masked off before the section number is used as a bitset
        // index. Model streams carry face 7 there, and without the mask such a group would read
        // far past the end of the array.
        let sec = field(g.section, SECTION_INDEX_SHIFT, SECTION_INDEX_BITS);
        let reachable = (cave_visible[sec >> 5u] >> (sec & 31u)) & 1u;
        // Model geometry hangs outside the section that owns it — a fence arm, a rail on a slope —
        // so both tests below run against a box grown by however far this stream can reach. Without
        // it a section on the very edge of the frustum takes the quad poking into frame with it.
        let origin = section_min(g);
        let mn = origin - params.overhang;
        let mx = origin + f32(SECTION_SIZE) + params.overhang;
        let face = field(g.section, GROUP_FACE_SHIFT, GROUP_FACE_BITS);
        if (reachable != 0u && in_frustum(mn, mx) && faces_camera(face, mn, mx)) {
            reserved_slot = atomicAdd(&args[params.args_index].instance_count, g.quad_count);
        } else {
            reserved_slot = CULLED;
        }
    }
    workgroupBarrier();

    let base = reserved_slot;
    if (base == CULLED) {
        return;
    }
    var i = local;
    loop {
        if (i >= g.quad_count) {
            break;
        }
        visible[params.visible_base + base + i] = g.quad_base + i;
        i = i + 64u;
    }
}
