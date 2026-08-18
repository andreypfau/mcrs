// Frustum and backface-group culling for one geometry stream.
//
// One workgroup handles one group — a run of quads from a single section and face group. Thread 0
// decides whether the whole run survives and reserves a contiguous span in the visible list with a
// single atomic; the other 63 threads then scatter the quad indices into it. That keeps the atomic
// traffic at one operation per run rather than one per quad, and the draw that follows is a single
// `draw_indirect` whose instance count the same atomic produced.

#import bevy_render::view::View

struct Group {
    quad_base: u32,
    quad_count: u32,
    // sx | sy << 5 | sz << 10 | face << 15, section coordinates in units of 16 blocks.
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

var<workgroup> reserved_slot: u32;

fn section_min(g: Group) -> vec3<f32> {
    let sx = i32(g.section & 31u);
    let sy = i32((g.section >> 5u) & 31u) + params.min_section_y;
    let sz = i32((g.section >> 10u) & 31u);
    return vec3<f32>(f32(sx * 16), f32(sy * 16), f32(sz * 16));
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
        // Bits 15..17 are the face group; the section number is the low 15 bits, which is exactly
        // the bitset's index space. Model streams pack FACE_NONE = 7 in there, so the mask is
        // mandatory: without it such a group would read the 7168th word of a 1024-word array.
        let sec = g.section & 0x7fffu;
        let reachable = (cave_visible[sec >> 5u] >> (sec & 31u)) & 1u;
        // Model geometry hangs outside the section that owns it — a fence arm, a rail on a slope —
        // so both tests below run against a box grown by however far this stream can reach. Without
        // it a section on the very edge of the frustum takes the quad poking into frame with it.
        let origin = section_min(g);
        let mn = origin - params.overhang;
        let mx = origin + 16.0 + params.overhang;
        let face = (g.section >> 15u) & 7u;
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
