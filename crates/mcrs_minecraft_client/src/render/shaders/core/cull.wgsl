
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

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

@group(1) @binding(0) var<storage, read> groups: array<Group>;
@group(1) @binding(1) var<storage, read_write> visible: array<vec2<u32>>;
@group(1) @binding(2) var<storage, read_write> args: array<DrawArgs>;
@group(1) @binding(3) var<storage, read> cave_visible: array<u32>;
@group(1) @binding(4) var<storage, read> sections: array<SectionDesc>;

/// The dispatch does not scale with the world: a fixed grid of workgroups strides over the group
/// arena, so a resident world of any size is covered by the same command and the 65535-per-
/// dimension cap on a dispatch can never be reached.
///
/// Within a group the work splits in two. One thread decides the group's fate and reserves its
/// run of the visible list; the whole workgroup then writes that run. The reservation crosses
/// between the two in workgroup memory, so neither a second pass nor a side buffer is needed.
const CULL_THREADS: u32 = 32u;

var<workgroup> reserved_slot: u32;

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

@compute @workgroup_size(CULL_THREADS)
fn cull(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var slot = workgroup.x;
    loop {
        if (slot >= params.group_count) {
            break;
        }
        let g = groups[params.group_base + slot];

        if (local == 0u) {
            if (survives(g)) {
                reserved_slot = atomicAdd(&args[params.args_index].instance_count, g.quad_count);
            } else {
                reserved_slot = CULLED;
            }
        }
        workgroupBarrier();

        let base = reserved_slot;
        if (base != CULLED) {
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
        slot = slot + grid.x;
    }
}

/// Blended geometry: every group keeps the slot the mesher gave it and culled quads leave a
/// hole, because packing would reshuffle the back-to-front order the blend depends on.
@compute @workgroup_size(CULL_THREADS)
fn cull_stable(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var slot = workgroup.x;
    loop {
        if (slot >= params.group_count) {
            break;
        }
        let g = groups[params.group_base + slot];

        if (local == 0u) {
            let lives = survives(g);
            if (lives) {
                atomicMax(&args[params.args_index].instance_count, g.quad_prefix + g.quad_count);
            }
            reserved_slot = select(CULLED, g.quad_prefix, lives);
        }
        workgroupBarrier();

        let culled = reserved_slot == CULLED;
        var i = local;
        loop {
            if (i >= g.quad_count) {
                break;
            }
            visible[params.visible_base + g.quad_prefix + i] =
                vec2<u32>(select(g.quad_base + i, CULLED, culled), g.section);
            i = i + CULL_THREADS;
        }
        workgroupBarrier();
        slot = slot + grid.x;
    }
}
