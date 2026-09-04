
#import mcrs_minecraft_client::fields::FACE_NONE
#import mcrs_minecraft_client::frame::{camera, params}
#import mcrs_minecraft_client::quad::face_normal
#import mcrs_minecraft_client::section::{SectionDesc, section_origin, section_span}

struct Group {
    quad_base: u32,
    quad_count: u32,
    section: u32,
    face: u32,
    quad_prefix: u32,
}

struct DrawArgs {
    index_count: atomic<u32>,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

const INDICES_PER_QUAD: u32 = 6u;

@group(1) @binding(0) var<storage, read> groups: array<Group>;
@group(1) @binding(1) var<storage, read_write> visible: array<vec2<u32>>;
@group(1) @binding(2) var<storage, read_write> args: array<DrawArgs>;
@group(1) @binding(3) var<storage, read> cave_visible: array<u32>;
@group(1) @binding(4) var<storage, read> sections: array<SectionDesc>;
@group(1) @binding(5) var<storage, read_write> batches: array<u32>;
@group(1) @binding(6) var hiz: texture_2d<f32>;
/// One word a group: whether the first pass left it to the second, hidden by the last frame's
/// depth alone.
@group(1) @binding(7) var<storage, read_write> candidates: array<u32>;

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

/// Whether the last frame drew something nearer than every point of the box over the whole of
/// its footprint. A box reaching behind the camera or past the edge of the screen is never
/// hidden: what the last frame did not draw says nothing about it.
fn behind_terrain(mn: vec3<f32>, mx: vec3<f32>) -> bool {
    var lo = vec2<f32>(1e30);
    var hi = vec2<f32>(-1e30);
    var nearest = 0.0;
    for (var i = 0u; i < 8u; i = i + 1u) {
        let corner = vec3<f32>(
            select(mn.x, mx.x, (i & 1u) != 0u),
            select(mn.y, mx.y, (i & 2u) != 0u),
            select(mn.z, mx.z, (i & 4u) != 0u),
        );
        let clip = camera.clip_from_relative * vec4<f32>(corner, 1.0);
        if (clip.w <= 0.0) {
            return false;
        }
        let ndc = clip.xyz / clip.w;
        lo = min(lo, ndc.xy);
        hi = max(hi, ndc.xy);
        nearest = max(nearest, ndc.z);
    }
    if (any(lo < vec2<f32>(-1.0)) || any(hi > vec2<f32>(1.0))) {
        return false;
    }
    let uv_lo = vec2<f32>(lo.x, -hi.y) * 0.5 + 0.5;
    let uv_hi = vec2<f32>(hi.x, -lo.y) * 0.5 + 0.5;
    let base = vec2<f32>(textureDimensions(hiz, 0));
    let px = (uv_hi - uv_lo) * base;
    let level = min(u32(ceil(log2(max(max(px.x, px.y), 1.0)))), camera.hiz_levels - 1u);
    let size = textureDimensions(hiz, level);
    let last = size - vec2<u32>(1u);
    let t_lo = min(vec2<u32>(uv_lo * vec2<f32>(size)), last);
    let t_hi = min(vec2<u32>(uv_hi * vec2<f32>(size)), last);
    var farthest = 1.0;
    for (var y = t_lo.y; y <= t_hi.y; y = y + 1u) {
        for (var x = t_lo.x; x <= t_hi.x; x = x + 1u) {
            farthest = min(farthest, textureLoad(hiz, vec2<u32>(x, y), level).r);
        }
    }
    return nearest < farthest;
}

/// Loads this lane's group of the batch starting at `first` and answers whether it is drawn.
/// A group the last frame's depth alone hides is left to the second pass, and counted against
/// the draw's counter when asked.
fn load(first: u32, local: u32, tally: bool) -> bool {
    let slot = first + local;
    if (slot >= params.group_count) {
        return false;
    }
    let g = groups[params.group_base + slot];
    batch[local] = g;
    candidates[params.group_base + slot] = 0u;
    if (g.quad_count == 0u || !survives(g)) {
        return false;
    }
    let desc = sections[g.section];
    let origin = section_origin(desc);
    if (behind_terrain(origin, origin + section_span(desc))) {
        candidates[params.group_base + slot] = 1u;
        if (tally) {
            atomicAdd(&args[params.counter].index_count, g.quad_count);
        }
        return false;
    }
    return true;
}

/// Loads this lane's group for the second pass: one the first pass left, tested again against
/// the depth this frame has drawn so far.
fn load_candidate(first: u32, local: u32) -> bool {
    let slot = first + local;
    if (slot >= params.group_count || candidates[params.group_base + slot] == 0u) {
        return false;
    }
    let g = groups[params.group_base + slot];
    batch[local] = g;
    let desc = sections[g.section];
    let origin = section_origin(desc);
    return !behind_terrain(origin, origin + section_span(desc));
}

/// The second pass packs what it revives after the first pass's survivors, in the same slots
/// of the visible list, and draws them from a second set of args whose first index starts
/// where the first draw's quads end.
@compute @workgroup_size(CULL_THREADS)
fn cull_second(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    let drawn = atomicLoad(&args[params.args_index].index_count) / INDICES_PER_QUAD;
    let second = params.args_index + STREAMS;
    if (workgroup.x == 0u && local == 0u) {
        args[second].first_index = drawn * INDICES_PER_QUAD;
    }
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= params.group_count) {
            break;
        }
        let lives = load_candidate(first, local);
        counts[local] = select(0u, batch[local].quad_count, lives);
        workgroupBarrier();

        if (local == 0u) {
            var total = 0u;
            for (var k = 0u; k < CULL_THREADS; k = k + 1u) {
                starts[k] = total;
                total = total + counts[k];
            }
            reserved = drawn
                + atomicAdd(&args[second].index_count, total * INDICES_PER_QUAD) / INDICES_PER_QUAD;
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
        let lives = load(first, local, true);
        counts[local] = select(0u, batch[local].quad_count, lives);
        workgroupBarrier();

        if (local == 0u) {
            var total = 0u;
            for (var k = 0u; k < CULL_THREADS; k = k + 1u) {
                starts[k] = total;
                total = total + counts[k];
            }
            reserved = atomicAdd(&args[params.args_index].index_count, total * INDICES_PER_QUAD)
                / INDICES_PER_QUAD;
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

/// Blended geometry keeps the back-to-front order the blend depends on, so its survivors are
/// packed by a prefix sum over the batches rather than by an atomic: `count_ordered` writes each
/// batch's surviving quads, `scan_ordered` turns them into where each batch starts, and
/// `scatter_ordered` writes the quads there. A batch's index is its first group's slot in the
/// arena over `CULL_THREADS`, which a stream's block, aligned to a power of two, keeps exact.
@compute @workgroup_size(CULL_THREADS)
fn count_ordered(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= params.group_count) {
            break;
        }
        let lives = load(first, local, true);
        counts[local] = select(0u, batch[local].quad_count, lives);
        workgroupBarrier();
        if (local == 0u) {
            var total = 0u;
            for (var k = 0u; k < CULL_THREADS; k = k + 1u) {
                total = total + counts[k];
            }
            batches[(params.group_base + first) / CULL_THREADS] = total;
        }
        workgroupBarrier();
        first = first + grid.x * CULL_THREADS;
    }
}

var<workgroup> partial: array<u32, CULL_THREADS>;

/// One workgroup per ordered draw: each lane scans a run of its batches, the runs are joined,
/// and the draw's instance count becomes what survived in all.
@compute @workgroup_size(CULL_THREADS)
fn scan_ordered(@builtin(local_invocation_index) local: u32) {
    let first = params.group_base / CULL_THREADS;
    let count = (params.group_count + CULL_THREADS - 1u) / CULL_THREADS;
    let run = (count + CULL_THREADS - 1u) / CULL_THREADS;
    let lo = min(local * run, count);
    let hi = min(lo + run, count);
    var sum = 0u;
    for (var b = lo; b < hi; b = b + 1u) {
        sum = sum + batches[first + b];
    }
    partial[local] = sum;
    workgroupBarrier();
    var running = 0u;
    for (var k = 0u; k < local; k = k + 1u) {
        running = running + partial[k];
    }
    for (var b = lo; b < hi; b = b + 1u) {
        let here = batches[first + b];
        batches[first + b] = running;
        running = running + here;
    }
    if (local == CULL_THREADS - 1u) {
        atomicStore(&args[params.args_index].index_count, running * INDICES_PER_QUAD);
    }
}

@compute @workgroup_size(CULL_THREADS)
fn scatter_ordered(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * CULL_THREADS;
    loop {
        if (first >= params.group_count) {
            break;
        }
        let lives = load(first, local, false);
        counts[local] = select(0u, batch[local].quad_count, lives);
        workgroupBarrier();
        if (local == 0u) {
            var total = batches[(params.group_base + first) / CULL_THREADS];
            for (var k = 0u; k < CULL_THREADS; k = k + 1u) {
                starts[k] = total;
                total = total + counts[k];
            }
        }
        workgroupBarrier();

        let in_batch = min(CULL_THREADS, params.group_count - first);
        for (var k = 0u; k < in_batch; k = k + 1u) {
            if (counts[k] == 0u) {
                continue;
            }
            let g = batch[k];
            var i = local;
            loop {
                if (i >= g.quad_count) {
                    break;
                }
                visible[params.visible_base + starts[k] + i] = vec2<u32>(g.quad_base + i, g.section);
                i = i + CULL_THREADS;
            }
        }
        workgroupBarrier();
        first = first + grid.x * CULL_THREADS;
    }
}
