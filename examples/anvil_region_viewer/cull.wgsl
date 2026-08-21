#import bevy_render::view::View
#import anvil_region_viewer::layout::{
    CULLED, Params, SECTION_SIZE, section_origin,
}

const SECTION_INDEX_WORD: u32 = 0u;
const SECTION_INDEX_SHIFT: u32 = 0u;
const SECTION_INDEX_BITS: u32 = 11u;
const GROUP_FACE_WORD: u32 = 0u;
const GROUP_FACE_SHIFT: u32 = 11u;
const GROUP_FACE_BITS: u32 = 4u;

struct Group {
    quad_base: u32,
    quad_count: u32,
    section: u32,
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

const FACE_NONE: u32 = 10u;

fn faces_camera(face: u32, mn: vec3<f32>, mx: vec3<f32>) -> bool {
    if (face >= FACE_NONE) {
        return true;
    }
    let n = group_normal(face);
    let nearest = select(mx, mn, n > vec3<f32>(0.0));
    return dot(n, view.world_position.xyz - nearest) > 0.0;
}

fn survives(g: Group) -> bool {
    let sec = params.cave_base + extractBits(g.section, SECTION_INDEX_SHIFT, SECTION_INDEX_BITS);
    let reachable = (cave_visible[sec >> 5u] >> (sec & 31u)) & 1u;
    let origin = section_min(g);
    let mn = origin - params.overhang;
    let mx = origin + SECTION_SIZE + params.overhang;
    let face = extractBits(g.section, GROUP_FACE_SHIFT, GROUP_FACE_BITS);
    return reachable != 0u && in_frustum(mn, mx) && faces_camera(face, mn, mx);
}

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
