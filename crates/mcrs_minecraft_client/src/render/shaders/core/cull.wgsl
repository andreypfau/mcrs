
#import mcrs_minecraft_client::fields::FACE_NONE
#import mcrs_minecraft_client::frame::{camera, params}
#import mcrs_minecraft_client::quad::face_normal
#import mcrs_minecraft_client::section::{CULLED, SectionDesc, section_origin, section_span}

struct Group {
    quad_base: u32,
    quad_count: u32,
    section: u32,
    face: u32,
    quad_prefix: u32,
}

/// One entry per draw, then one counter entry per draw: a counter's `instance_count` is the
/// number of quads that survived and its `first_instance` the lowest slot one of them keeps.
struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: atomic<u32>,
}

@group(1) @binding(0) var<storage, read> groups: array<Group>;
@group(1) @binding(1) var<storage, read_write> visible: array<vec2<u32>>;
@group(1) @binding(2) var<storage, read_write> args: array<DrawArgs>;
@group(1) @binding(3) var<storage, read> cave_visible: array<u32>;
@group(1) @binding(4) var<storage, read> sections: array<SectionDesc>;

/// The dispatch does not scale with the world: a fixed grid of workgroups strides over the group
/// arena a batch of `CULL_THREADS` groups at a time, so a resident world of any size is covered
/// by the same command and the 65535-per-dimension cap on a dispatch can never be reached.
///
/// Within a batch every lane tests its own group, lane 0 sums what survived and reserves the
/// batch's run of the visible list with one atomic, and the whole workgroup then writes each
/// surviving group's quads.
const CULL_THREADS: u32 = #{CULL_THREADS}u;
const STREAMS: u32 = #{STREAMS}u;
const NO_SLOT: u32 = 0xffffffffu;

var<workgroup> batch: array<Group, CULL_THREADS>;
var<workgroup> counts: array<u32, CULL_THREADS>;
var<workgroup> starts: array<u32, CULL_THREADS>;
var<workgroup> reserved: u32;

fn in_frustum(mn: vec3<f32>, mx: vec3<f32>) -> bool {
    for (var i = 0u; i < 5u; i = i + 1u) {
        let plane = camera.frustum[i];
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

fn faces_camera(face: u32, mn: vec3<f32>, mx: vec3<f32>) -> bool {
    if (face >= FACE_NONE) {
        return true;
    }
    let n = face_normal(face);
    let nearest = select(mx, mn, n > vec3<f32>(0.0));
    return dot(n, camera.offset - nearest) > 0.0;
}

fn survives(g: Group) -> bool {
    let reachable = (cave_visible[g.section >> 5u] >> (g.section & 31u)) & 1u;
    let desc = sections[g.section];
    let origin = section_origin(desc);
    let mn = origin - params.overhang;
    let mx = origin + section_span(desc) + params.overhang;
    return reachable != 0u && in_frustum(mn, mx) && faces_camera(g.face, mn, mx);
}

/// Loads this lane's group of the batch starting at `first` and answers whether it is drawn.
fn load(first: u32, local: u32) -> bool {
    let slot = first + local;
    if (slot >= params.group_count) {
        return false;
    }
    let g = groups[params.group_base + slot];
    batch[local] = g;
    return survives(g);
}

@compute @workgroup_size(CULL_THREADS)
fn cull(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= params.group_count) {
            break;
        }
        let lives = load(first, local);
        counts[local] = select(0u, batch[local].quad_count, lives);
        workgroupBarrier();

        if (local == 0u) {
            var total = 0u;
            for (var k = 0u; k < CULL_THREADS; k = k + 1u) {
                starts[k] = total;
                total = total + counts[k];
            }
            reserved = atomicAdd(&args[params.args_index].instance_count, total);
        }
        workgroupBarrier();

        let in_batch = min(CULL_THREADS, params.group_count - first);
        for (var k = 0u; k < in_batch; k = k + 1u) {
            if (counts[k] == 0u) {
                continue;
            }
            let g = batch[k];
            let base = reserved + starts[k];
            var i = local;
            loop {
                if (i >= g.quad_count) {
                    break;
                }
                visible[params.visible_base + base + i] = vec2<u32>(g.quad_base + i, g.section);
                i = i + CULL_THREADS;
            }
        }
        workgroupBarrier();
        first = first + grid.x * CULL_THREADS;
    }
}

/// Blended geometry: every group keeps the slot the mesher gave it and culled quads leave a
/// hole, because packing would reshuffle the back-to-front order the blend depends on. The draw
/// covers the slots from the first survivor to the last, so what lies outside that range costs
/// nothing however much of it is resident.
@compute @workgroup_size(CULL_THREADS)
fn cull_stable(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= params.group_count) {
            break;
        }
        counts[local] = u32(load(first, local));
        workgroupBarrier();

        let in_batch = min(CULL_THREADS, params.group_count - first);
        if (local == 0u) {
            var top = 0u;
            var bottom = NO_SLOT;
            var total = 0u;
            for (var k = 0u; k < in_batch; k = k + 1u) {
                if (counts[k] != 0u) {
                    top = max(top, batch[k].quad_prefix + batch[k].quad_count);
                    bottom = min(bottom, batch[k].quad_prefix);
                    total = total + batch[k].quad_count;
                }
            }
            if (total != 0u) {
                atomicMax(&args[params.args_index].instance_count, top);
                atomicMin(&args[params.counter].first_instance, bottom);
                atomicAdd(&args[params.counter].instance_count, total);
            }
        }

        for (var k = 0u; k < in_batch; k = k + 1u) {
            let g = batch[k];
            let culled = counts[k] == 0u;
            var i = local;
            loop {
                if (i >= g.quad_count) {
                    break;
                }
                visible[params.visible_base + g.quad_prefix + i] =
                    vec2<u32>(select(g.quad_base + i, CULLED, culled), g.section);
                i = i + CULL_THREADS;
            }
        }
        workgroupBarrier();
        first = first + grid.x * CULL_THREADS;
    }
}

/// Turns each ordered draw's end slot into a count from its first survivor. A packed draw's
/// counter keeps a zero start and is left as it stands.
@compute @workgroup_size(CULL_THREADS)
fn finalize(@builtin(local_invocation_index) local: u32) {
    if (local >= STREAMS) {
        return;
    }
    let first = atomicLoad(&args[STREAMS + local].first_instance);
    if (first == NO_SLOT) {
        return;
    }
    atomicSub(&args[local].instance_count, first);
}
