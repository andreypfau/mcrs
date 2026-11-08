// The expand pass of one client entity definition: one lane for each visible mob and each
// cuboid of its active layers. A lane writes the face records of the faces that its cuboid
// shows, in the order in which the rasterizer must draw them.

const DEFINITION: u32 = #{DEFINITION}u;
const LANES: u32 = #{LANES}u;

var<workgroup> group_faces: array<atomic<u32>, 8>;
var<workgroup> group_faces_base: array<u32, 8>;
var<workgroup> group_cutout_faces: array<atomic<u32>, 8>;
var<workgroup> group_cutout_base: array<u32, 8>;

/// A cuboid of the active layers of a slot.
struct Found {
    found: bool,
    layer: u32,
    cuboid: u32,
    model: ModelRecord,
    variant: u32,
    first_bone: u32,
}

fn find_cuboid(state: StateRecord, first_bone: u32, lane: u32) -> Found {
    var result: Found;
    result.found = false;
    var rest = lane;
    var bone = first_bone;
    let mask = layer_mask(state);
    for (var layer = 0u; layer < 4u; layer = layer + 1u) {
        if ((mask & (1u << layer)) == 0u) {
            continue;
        }
        let word = layer_word(state, layer);
        let model = models[word & 0xffffu];
        if (rest < model.cuboid_count) {
            result.found = true;
            result.layer = layer;
            result.cuboid = rest;
            result.model = model;
            result.variant = word >> 16u;
            result.first_bone = bone;
            return result;
        }
        rest = rest - model.cuboid_count;
        bone = bone + model.bone_count;
    }
    return result;
}

fn compose(instance: Instance, bone: u32) -> Placed {
    return compose_rows(instance, bone_matrices[bone * 3u], bone_matrices[bone * 3u + 1u], bone_matrices[bone * 3u + 2u]);
}

fn faces_camera(placed: Placed, normal: vec3<f32>, centre: vec3<f32>) -> bool {
    let world_normal = transform_normal(placed.row0, placed.row1, placed.row2, normal);
    let world_centre = transform_point(placed.row0, placed.row1, placed.row2, centre);
    return dot(world_normal, frame.camera_offset - world_centre) > 0.0;
}

fn cutout_bits(variant: u32, model: ModelRecord, cuboid: u32) -> u32 {
    let byte = variants[variant].cutout_first + model.first_cuboid + cuboid;
    return (cutout[byte >> 2u] >> ((byte & 3u) * 8u)) & 0x3fu;
}

fn opposite(face: u32) -> u32 {
    switch face {
        case 0u: { return 2u; }
        case 1u: { return 3u; }
        case 2u: { return 0u; }
        case 3u: { return 1u; }
        case 4u: { return 5u; }
        default: { return 4u; }
    }
}

@compute @workgroup_size(64)
fn expand(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    if (local < ARRAYS) {
        atomicStore(&group_faces[local], 0u);
        atomicStore(&group_cutout_faces[local], 0u);
    }
    workgroupBarrier();
    var emitted = array<u32, 6>();
    var emit_count = 0u;
    var target_array = 0u;
    var index = 0u;
    var packed = 0u;
    var cut = 0u;
    let count = min(atomicLoad(&counters[COUNTER_VISIBLE + DEFINITION]), frame.visible_capacity);
    let mob = id.x / LANES;
    if (mob < count) {
        index = DEFINITION * frame.visible_capacity + mob;
        let entry = visible[index];
        if (entry.x != NO_SLOT) {
            let state = states[entry.x];
            let found = find_cuboid(state, entry.y, id.x % LANES);
            if (found.found) {
                let at = found.model.first_cuboid + found.cuboid;
                let cuboid = cuboids[at];
                let bone_index = cuboid_bone(cuboid);
                let bone = bones[found.model.first_bone + bone_index];
                let palette = bone.parent_palette >> 16u;
                let hidden_mask = state.flags >> STATE_HIDDEN_SHIFT;
                let hidden = (bone.flags & BONE_NEVER_RENDER) != 0u
                    || (palette < 16u && ((hidden_mask >> palette) & 1u) != 0u);
                if (!hidden) {
                    let placed = compose(instances[index], found.first_bone + bone_index);
                    let flags = cuboid_flags(cuboid);
                    let face_mask = flags & 0x3fu;
                    cut = cutout_bits(found.variant, found.model, found.cuboid);
                    target_array = variants[found.variant].array_layer >> 16u;
                    packed = found.cuboid | (found.layer << 16u);
                    if ((flags & CUBOID_EXPLICIT) != 0u) {
                        let first_face = bitcast<u32>(cuboid.origin_x);
                        var ordinal = 0u;
                        for (var face = 0u; face < 6u; face = face + 1u) {
                            if ((face_mask & (1u << face)) == 0u) {
                                continue;
                            }
                            let slot_a = cuboids[found.model.first_cuboid + found.model.cuboid_count + 2u * (first_face + ordinal)];
                            let corner = vec3<f32>(slot_a.origin_x, slot_a.origin_y, slot_a.origin_z);
                            let edge_1 = vec3<f32>(slot_a.size_x, slot_a.size_y, slot_a.size_z);
                            let slot_b = cuboids[found.model.first_cuboid + found.model.cuboid_count + 2u * (first_face + ordinal) + 1u];
                            let edge_2 = vec3<f32>(bitcast<f32>(slot_a.uv), bitcast<f32>(slot_a.inflate_bone_flags), slot_b.origin_x);
                            let normal = normalize(cross(edge_1, edge_2));
                            let centre = corner + 0.5 * (edge_1 + edge_2);
                            if (cut != 0u || faces_camera(placed, normal, centre)) {
                                emitted[emit_count] = face;
                                emit_count = emit_count + 1u;
                            }
                            ordinal = ordinal + 1u;
                        }
                    } else {
                        let size = cuboid_size(cuboid);
                        let box = drawn_box(cuboid);
                        let centre = (box.lo + box.hi) * 0.5;
                        let half = (box.hi - box.lo) * 0.5;
                        let zero = size == vec3<f32>(0.0);
                        var selected = 6u;
                        if (zero.x && !zero.y && !zero.z) {
                            selected = select(FACE_WEST, FACE_EAST, (flags & CUBOID_MIRROR) != 0u);
                        } else if (zero.y && !zero.x && !zero.z) {
                            selected = FACE_DOWN;
                        } else if (zero.z && !zero.x && !zero.y) {
                            selected = FACE_SOUTH;
                        }
                        if (selected < 6u) {
                            let other = opposite(selected);
                            if (((cut >> selected) & 1u) != 0u && (face_mask & (1u << other)) != 0u) {
                                emitted[emit_count] = other;
                                emit_count = emit_count + 1u;
                            }
                            if ((face_mask & (1u << selected)) != 0u) {
                                emitted[emit_count] = selected;
                                emit_count = emit_count + 1u;
                            }
                        } else if (!any(zero)) {
                            for (var face = 0u; face < 6u; face = face + 1u) {
                                if ((face_mask & (1u << face)) == 0u) {
                                    continue;
                                }
                                let normal = face_normal(face);
                                if (cut != 0u || faces_camera(placed, normal, centre + normal * half)) {
                                    emitted[emit_count] = face;
                                    emit_count = emit_count + 1u;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    target_array = min(target_array, ARRAYS - 1u);
    // A face with a cutout texel draws with the discard; the others draw with no discard, so a
    // tile-based GPU removes their hidden fragments.
    var opaque_count = 0u;
    for (var face = 0u; face < emit_count; face = face + 1u) {
        opaque_count = opaque_count + select(1u, 0u, ((cut >> emitted[face]) & 1u) != 0u);
    }
    let cutout_count = emit_count - opaque_count;
    var offset = 0u;
    var cutout_offset = 0u;
    if (opaque_count != 0u) {
        offset = atomicAdd(&group_faces[target_array], opaque_count);
    }
    if (cutout_count != 0u) {
        cutout_offset = atomicAdd(&group_cutout_faces[target_array], cutout_count);
    }
    workgroupBarrier();
    if (local < ARRAYS) {
        let total = atomicLoad(&group_faces[local]);
        if (total != 0u) {
            group_faces_base[local] = atomicAdd(&counters[COUNTER_FACES + local], total);
        }
        let cutout_total = atomicLoad(&group_cutout_faces[local]);
        if (cutout_total != 0u) {
            let from_end = atomicAdd(&counters[COUNTER_CUTOUT_FACES + local], cutout_total) + cutout_total;
            group_cutout_base[local] = frame.face_capacity - min(from_end, frame.face_capacity);
        }
    }
    workgroupBarrier();
    let region = target_array * frame.face_capacity;
    var opaque_at = group_faces_base[target_array] + offset;
    var cutout_at = group_cutout_base[target_array] + cutout_offset;
    for (var face = 0u; face < emit_count; face = face + 1u) {
        let record = vec2<u32>(index, packed | (emitted[face] << 24u));
        if (((cut >> emitted[face]) & 1u) != 0u) {
            if (cutout_at < frame.face_capacity) {
                faces[region + cutout_at] = record;
            }
            cutout_at = cutout_at + 1u;
        } else {
            if (opaque_at < frame.face_capacity) {
                faces[region + opaque_at] = record;
            }
            opaque_at = opaque_at + 1u;
        }
    }
}
