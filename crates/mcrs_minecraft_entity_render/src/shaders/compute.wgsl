// The bindings of the entity compute passes. A pass binds only those its entry point reads.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read_write> ticks: array<TickRecord>;
@group(0) @binding(2) var<storage, read_write> previous_ticks: array<TickRecord>;
@group(0) @binding(3) var<storage, read_write> tick_of_record: array<u32>;
@group(0) @binding(4) var<storage, read> culls: array<CullRecord>;
@group(0) @binding(5) var<storage, read> states: array<StateRecord>;
@group(0) @binding(6) var<storage, read> inputs: array<InputRecord>;
@group(0) @binding(8) var<storage, read> models: array<ModelRecord>;
@group(0) @binding(9) var<storage, read> bones: array<BoneRecord>;
@group(0) @binding(10) var<storage, read> cuboids: array<CuboidRecord>;
@group(0) @binding(11) var<storage, read_write> visible: array<vec2<u32>>;
@group(0) @binding(12) var<storage, read_write> bone_matrices: array<vec4<f32>>;
@group(0) @binding(13) var<storage, read_write> instances: array<Instance>;
@group(0) @binding(14) var<storage, read_write> faces: array<vec2<u32>>;
@group(0) @binding(15) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(16) var<storage, read_write> indirect: array<u32>;
@group(0) @binding(17) var<storage, read> scatter: array<ScatterEntry>;
@group(0) @binding(18) var hiz: texture_2d<f32>;
@group(0) @binding(19) var<storage, read> variants: array<Variant>;
@group(0) @binding(20) var<storage, read> cutout: array<u32>;
@group(0) @binding(21) var<storage, read_write> impostors: array<u32>;
@group(0) @binding(22) var<storage, read> impostor_pairs: array<u32>;
@group(0) @binding(23) var<storage, read> impostor_frames: array<vec4<f32>>;
@group(0) @binding(24) var<storage, read_write> impostor_draws: array<ImpostorDraw>;
@group(0) @binding(25) var<storage, read> impostor_covers: array<vec4<f32>>;

struct ScatterEntry {
    slot: u32,
    tick: u32,
    record: TickRecord,
}

const WORKGROUP: u32 = 64u;
/// The bit of a scatter tick for a record with no previous one: both records take it.
const SCATTER_FRESH: u32 = 0x80000000u;
const DEFINITIONS: u32 = #{DEFINITIONS}u;
const ARRAYS: u32 = #{ARRAYS}u;
const EXPAND_LANES = #{EXPAND_LANES};
/// The height of the entity box at the scale 1 and the baby scale of each definition.
const BOX_SIZES = #{BOX_SIZES};
const VARIANTS: u32 = #{VARIANTS}u;
const COUNTER_BONES: u32 = 0u;
const COUNTER_VISIBLE: u32 = 1u;
const COUNTER_FACES: u32 = 17u;
const COUNTER_DROPPED: u32 = 25u;
const NO_SLOT: u32 = 0xffffffffu;
const COUNTER_IMPOSTORS: u32 = 26u;
/// The impostors that draw with the discard of the uncovered pixels, and the small ones that
/// draw their whole quad.
const COUNTER_CUTOUT_IMPOSTORS: u32 = 27u;
const COUNTER_SMALL_IMPOSTORS: u32 = 28u;
/// The face records of each texture array with a cutout texel, which fill its range from the end.
const COUNTER_CUTOUT_FACES: u32 = 29u;
const NO_IMPOSTOR: u32 = 0xffffffffu;

/// The scatter of the tick records that the CPU wrote since the last frame: the current record
/// of each slot becomes its previous record.
@compute @workgroup_size(64)
fn scatter_ticks(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= frame.scatter_count) {
        return;
    }
    let entry = scatter[id.x];
    let fresh = (entry.tick & SCATTER_FRESH) != 0u;
    if (fresh) {
        previous_ticks[entry.slot] = entry.record;
    } else {
        previous_ticks[entry.slot] = ticks[entry.slot];
    }
    ticks[entry.slot] = entry.record;
    tick_of_record[entry.slot] = entry.tick & ~SCATTER_FRESH;
}

var<workgroup> group_bones: atomic<u32>;
var<workgroup> group_bones_base: u32;
var<workgroup> group_visible: array<atomic<u32>, 16>;
var<workgroup> group_visible_base: array<u32, 16>;
var<workgroup> group_impostors: atomic<u32>;
var<workgroup> group_impostors_base: u32;

/// Whether each active layer of `state` has an impostor image set baked with its hidden bones.
fn impostor_ready(state: StateRecord) -> bool {
    let mask = layer_mask(state);
    let hidden = state.flags >> STATE_HIDDEN_SHIFT;
    for (var layer = 0u; layer < 4u; layer = layer + 1u) {
        if ((mask & (1u << layer)) == 0u) {
            continue;
        }
        let word = layer_word(state, layer);
        let variant = word >> 16u;
        let at = (word & 0xffffu) * VARIANTS + variant;
        if (variant >= VARIANTS || at >= arrayLength(&impostor_pairs)) {
            return false;
        }
        let pair = impostor_pairs[at];
        if (pair == NO_IMPOSTOR || (pair >> 16u) != hidden) {
            return false;
        }
    }
    return mask != 0u;
}

fn sphere_in_frustum(centre: vec3<f32>, radius: f32) -> bool {
    for (var i = 0u; i < 5u; i = i + 1u) {
        if (sphere_outside(frame.frustum[i], centre, radius)) {
            return false;
        }
    }
    return true;
}

fn sphere_hidden(centre: vec3<f32>, radius: f32) -> bool {
    if (frame.hiz_levels == 0u) {
        return false;
    }
    var footprint = empty_footprint();
    for (var corner = 0u; corner < 8u; corner = corner + 1u) {
        let offset = vec3<f32>(
            select(-radius, radius, (corner & 1u) != 0u),
            select(-radius, radius, (corner & 2u) != 0u),
            select(-radius, radius, (corner & 4u) != 0u),
        );
        cover(&footprint, frame.clip_from_relative, centre + offset);
    }
    return occluded(hiz, frame.viewport, frame.hiz_levels, footprint);
}

/// One lane for each slot. A visible slot under the height threshold goes to the impostor list.
/// Another visible slot reserves its bones and goes to the visible list of its definition; when
/// the bones or the list are full, it draws as an impostor.
@compute @workgroup_size(64)
fn cull(
    @builtin(workgroup_id) workgroup: vec3<u32>,
    @builtin(num_workgroups) grid: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    var first = workgroup.x * WORKGROUP;
    loop {
        if (first >= frame.slot_count) {
            break;
        }
        if (local == 0u) {
            atomicStore(&group_bones, 0u);
            atomicStore(&group_impostors, 0u);
        }
        if (local < 16u) {
            atomicStore(&group_visible[local], 0u);
        }
        workgroupBarrier();
        let slot = first + local;
        var keep = false;
        var ready = false;
        var impostor = false;
        var definition = 0u;
        var bone_count = 0u;
        if (slot < frame.slot_count) {
            let record = culls[slot];
            let flags = record.radius_flags >> 16u;
            if ((flags & CULL_VALID) != 0u) {
                let radius = unpack2x16float(record.radius_flags & 0xffffu).x + 0.8660254;
                let centre = relative_block(record.bx, record.by, record.bz) + vec3<f32>(0.5);
                keep = sphere_in_frustum(centre, radius) && !sphere_hidden(centre, radius);
                if (keep) {
                    let state = states[slot];
                    definition = state_definition(state);
                    let mask = layer_mask(state);
                    for (var layer = 0u; layer < 4u; layer = layer + 1u) {
                        if ((mask & (1u << layer)) != 0u) {
                            bone_count = bone_count + models[layer_word(state, layer) & 0xffffu].bone_count;
                        }
                    }
                    keep = definition < DEFINITIONS && bone_count != 0u;
                    if (keep) {
                        ready = (flags & CULL_FULL_RIG_ONLY) == 0u && impostor_ready(state);
                        var sizes = BOX_SIZES;
                        let size = sizes[definition];
                        let baby = select(1.0, size.y, (state.flags & STATE_BABY) != 0u);
                        let scale = unpack2x16float(state.light_scale >> 16u).x;
                        let distance = max(length(centre - frame.camera_offset), 1e-3);
                        let height = size.x * baby * scale * frame.projection_scale / distance;
                        impostor = ready && height < frame.height_threshold;
                    }
                }
            }
        }
        let full = keep && !impostor;
        var bone_offset = 0u;
        var visible_offset = 0u;
        if (full) {
            bone_offset = atomicAdd(&group_bones, bone_count);
            visible_offset = atomicAdd(&group_visible[definition], 1u);
        }
        workgroupBarrier();
        if (local == 0u) {
            group_bones_base = atomicAdd(&counters[COUNTER_BONES], atomicLoad(&group_bones));
        }
        if (local < DEFINITIONS) {
            let count = atomicLoad(&group_visible[local]);
            if (count != 0u) {
                group_visible_base[local] = atomicAdd(&counters[COUNTER_VISIBLE + local], count);
            }
        }
        workgroupBarrier();
        if (full) {
            let first_bone = group_bones_base + bone_offset;
            let index = group_visible_base[definition] + visible_offset;
            let fits = first_bone + bone_count <= frame.bone_capacity;
            if (index < frame.visible_capacity) {
                visible[definition * frame.visible_capacity + index] =
                    select(vec2<u32>(NO_SLOT), vec2<u32>(slot, first_bone), fits);
            }
            if (!fits || index >= frame.visible_capacity) {
                if (ready) {
                    impostor = true;
                } else {
                    atomicAdd(&counters[COUNTER_DROPPED], 1u);
                }
            }
        }
        var impostor_offset = 0u;
        if (impostor) {
            impostor_offset = atomicAdd(&group_impostors, 1u);
        }
        workgroupBarrier();
        if (local == 0u) {
            group_impostors_base = atomicAdd(&counters[COUNTER_IMPOSTORS], atomicLoad(&group_impostors));
        }
        workgroupBarrier();
        if (impostor) {
            let at = group_impostors_base + impostor_offset;
            if (at < arrayLength(&impostors)) {
                impostors[at] = slot;
            }
        }
        first = first + grid.x * WORKGROUP;
        workgroupBarrier();
    }
}

/// The dispatch of the pose and of the expand of each definition, from the visible counts.
@compute @workgroup_size(1)
fn dispatch_args() {
    for (var definition = 0u; definition < DEFINITIONS; definition = definition + 1u) {
        let count = min(atomicLoad(&counters[COUNTER_VISIBLE + definition]), frame.visible_capacity);
        indirect[definition * 3u] = (count + WORKGROUP - 1u) / WORKGROUP;
        indirect[definition * 3u + 1u] = 1u;
        indirect[definition * 3u + 2u] = 1u;
        var lanes_of = EXPAND_LANES;
        let lanes = count * lanes_of[definition];
        let expand = DEFINITIONS * 3u + definition * 3u;
        indirect[expand] = min((lanes + WORKGROUP - 1u) / WORKGROUP, 65535u);
        indirect[expand + 1u] = 1u;
        indirect[expand + 2u] = 1u;
    }
}

/// The draw of each texture array, from the face counts.
@compute @workgroup_size(1)
fn draw_args() {
    for (var layer_array = 0u; layer_array < ARRAYS; layer_array = layer_array + 1u) {
        let count = min(atomicLoad(&counters[COUNTER_FACES + layer_array]), frame.face_capacity);
        let draw = DEFINITIONS * 6u + layer_array * 4u;
        indirect[draw] = 4u;
        indirect[draw + 1u] = count;
        indirect[draw + 2u] = layer_array * frame.face_capacity * 4u;
        indirect[draw + 3u] = 0u;
        let cutout = min(atomicLoad(&counters[COUNTER_CUTOUT_FACES + layer_array]), frame.face_capacity);
        let cutout_draw = DEFINITIONS * 6u + ARRAYS * 4u + 12u + layer_array * 4u;
        indirect[cutout_draw] = 4u;
        indirect[cutout_draw + 1u] = cutout;
        indirect[cutout_draw + 2u] = ((layer_array + 1u) * frame.face_capacity - cutout) * 4u;
        indirect[cutout_draw + 3u] = 0u;
    }
    let impostor_draw = DEFINITIONS * 6u + ARRAYS * 4u;
    let impostors = min(atomicLoad(&counters[COUNTER_IMPOSTORS]), frame.slot_count);
    indirect[impostor_draw] = impostors * 6u;
    indirect[impostor_draw + 1u] = 1u;
    indirect[impostor_draw + 2u] = 0u;
    indirect[impostor_draw + 3u] = 0u;
    indirect[impostor_draw + 4u] = min((impostors + WORKGROUP - 1u) / WORKGROUP, 65535u);
    indirect[impostor_draw + 5u] = 1u;
    indirect[impostor_draw + 6u] = 1u;
}

/// The draws of the two kinds of impostors, after their prepare pass. The small impostors take
/// the draw records from the end of the slot range down.
@compute @workgroup_size(1)
fn impostor_draw_args() {
    let impostor_draw = DEFINITIONS * 6u + ARRAYS * 4u;
    indirect[impostor_draw] = atomicLoad(&counters[COUNTER_CUTOUT_IMPOSTORS]) * 6u;
    let small = atomicLoad(&counters[COUNTER_SMALL_IMPOSTORS]);
    let small_draw = impostor_draw + 8u;
    indirect[small_draw] = small * 6u;
    indirect[small_draw + 1u] = 1u;
    indirect[small_draw + 2u] = (frame.slot_count - small) * 6u;
    indirect[small_draw + 3u] = 0u;
}
