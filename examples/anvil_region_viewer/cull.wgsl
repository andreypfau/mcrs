// Frustum and backface-group culling for one geometry stream.
//
// One workgroup handles one group — a run of quads from a single section and face group. Thread 0
// decides whether the whole run survives and reserves a contiguous span in the visible list with a
// single atomic; the other 63 threads then scatter the quad indices into it. That keeps the atomic
// traffic at one operation per run rather than one per quad, and the draw that follows is a single
// `draw_indirect` whose instance count the same atomic produced.

#import bevy_render::view::View
#import anvil_region_viewer::layout::{
    CULLED, Params, SECTION_SIZE, section_origin,
}

// The packed geometry layout. The mesher writes these words and declares the same names, and a
// test reads both sides back and fails if a width or an offset ever drifts apart.
const SECTION_INDEX_WORD: u32 = 0u;
const SECTION_INDEX_SHIFT: u32 = 0u;
const SECTION_INDEX_BITS: u32 = 11u;
const GROUP_FACE_WORD: u32 = 0u;
const GROUP_FACE_SHIFT: u32 = 11u;
const GROUP_FACE_BITS: u32 = 4u;

struct Group {
    quad_base: u32,
    quad_count: u32,
    // The section this run came from, in units of 16 blocks, and the face group its quads point at.
    section: u32,
    // Where this run's quads start counting among the quads of its own draw, culled runs included.
    quad_prefix: u32,
}

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(1) @binding(0) var<storage, read> groups: array<Group>;
@group(1) @binding(1) var<storage, read_write> visible: array<u32>;
@group(1) @binding(2) var<storage, read_write> args: array<DrawArgs>;
@group(1) @binding(3) var<storage, read> cave_visible: array<u32>;

var<workgroup> reserved_slot: u32;

fn section_min(g: Group) -> vec3<f32> {
    let region = vec3<f32>(f32(params.origin_x), f32(params.origin_y), f32(params.origin_z));
    return section_origin(g.section, region);
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

/// The outward normal every quad of a face group shares. The six axes first, then the four
/// horizontal diagonals in the order the mesher numbers them — those are where the panes of a plant
/// point, and nowhere else. Lengths do not matter to the only caller, which compares against zero.
fn group_normal(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(0.0, -1.0, 0.0); }
        case 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 3u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 4u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 5u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 6u: { return vec3<f32>(1.0, 0.0, 1.0); }
        case 7u: { return vec3<f32>(1.0, 0.0, -1.0); }
        case 8u: { return vec3<f32>(-1.0, 0.0, 1.0); }
        default: { return vec3<f32>(-1.0, 0.0, -1.0); }
    }
}

/// The face group a stream carries when its quads point every which way and none of this applies.
const FACE_NONE: u32 = 10u;

/// Every quad in a face group shares one outward normal, so a group facing away from the camera is
/// entirely backfacing. At least three of the six axis groups fail this for any camera position.
///
/// A group survives when any point of its box could face the camera, which is `dot(n, camera)`
/// against the least `dot(n, p)` the box holds — and that least is reached at the corner taking
/// `mn` on the axes where the normal is positive and `mx` on the rest. Stated once rather than
/// solved by hand per group: the axes then cost the one comparison they always did, and the
/// diagonals stop being a second set of inequalities whose corner choice has to be got right twice.
fn faces_camera(face: u32, mn: vec3<f32>, mx: vec3<f32>) -> bool {
    if (face >= FACE_NONE) {
        return true;
    }
    let n = group_normal(face);
    let nearest = select(mx, mn, n > vec3<f32>(0.0));
    return dot(n, view.world_position.xyz - nearest) > 0.0;
}

/// Whether a run survives: its section reachable along a sight line, its box inside the frustum,
/// and its face group pointing anywhere the camera could see.
///
/// Model geometry hangs outside the section that owns it — a fence arm, a rail on a slope — so both
/// box tests run against a box grown by however far this stream can reach. Without it a section on
/// the very edge of the frustum takes the quad poking into frame with it.
fn survives(g: Group) -> bool {
    // The face group has to be masked off before the section number is used as a bitset index.
    // Model streams carry `FACE_NONE` there, and without the mask such a group would read far past
    // the end of the array. The section number is relative to the region, so the region's own base
    // has to be added back for the bitset the walk fills.
    let sec = params.cave_base + extractBits(g.section, SECTION_INDEX_SHIFT, SECTION_INDEX_BITS);
    let reachable = (cave_visible[sec >> 5u] >> (sec & 31u)) & 1u;
    let origin = section_min(g);
    let mn = origin - params.overhang;
    let mx = origin + SECTION_SIZE + params.overhang;
    let face = extractBits(g.section, GROUP_FACE_SHIFT, GROUP_FACE_BITS);
    return reachable != 0u && in_frustum(mn, mx) && faces_camera(face, mn, mx);
}

/// One SIMD group wide. Only one thread decides whether the run survives and the rest wait for it,
/// so a wider workgroup buys nothing and idles twice the lanes; at 64 the pass costs half again as
/// much. Giving one workgroup several runs to chew through is worse still — the scheduler is left
/// with fewer independent workgroups to spread over the machine.
const CULL_THREADS: u32 = 32u;

@compute @workgroup_size(CULL_THREADS)
fn cull(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    if (workgroup.x >= params.group_count) {
        return;
    }
    let g = groups[params.group_base + workgroup.x];

    if (local == 0u) {
        if (survives(g)) {
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
        i = i + CULL_THREADS;
    }
}

/// The same test, laying each run where its own place in the draw says rather than wherever the
/// atomic hands out.
///
/// Which matters only for blended geometry, and matters a great deal there: the order the visible
/// list holds is the order the quads are blended in, and the order the atomic hands out is whatever
/// the GPU's scheduler felt like this frame. A sea drawn in a different order every frame blends to
/// a different colour every frame. Opaque geometry has no such trouble — the depth test settles it
/// — and keeps the compacting pass above, because that one draws only what survived.
///
/// Nothing is compacted away here, so the draw covers every quad of the stream and the culled ones
/// are marked for the vertex stage to throw away as a quad with no area.
@compute @workgroup_size(CULL_THREADS)
fn cull_stable(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    if (workgroup.x >= params.group_count) {
        return;
    }
    let g = groups[params.group_base + workgroup.x];

    if (local == 0u) {
        // Every run reaches this, culled or not, so the largest end offset any of them names is the
        // whole stream's quad count — which is what the draw has to cover, and what no ordering
        // between the runs can change.
        atomicMax(&args[params.args_index].instance_count, g.quad_prefix + g.quad_count);
        reserved_slot = select(CULLED, g.quad_prefix, survives(g));
    }
    workgroupBarrier();

    let culled = reserved_slot == CULLED;
    var i = local;
    loop {
        if (i >= g.quad_count) {
            break;
        }
        visible[params.visible_base + g.quad_prefix + i] =
            select(g.quad_base + i, CULLED, culled);
        i = i + CULL_THREADS;
    }
}
