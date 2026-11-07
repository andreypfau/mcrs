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
const COUNTER_BONES: u32 = 0u;
const COUNTER_VISIBLE: u32 = 1u;
const COUNTER_FACES: u32 = 17u;
const COUNTER_DROPPED: u32 = 25u;
const NO_SLOT: u32 = 0xffffffffu;
/// The visible mobs under the height threshold, which draw a full rig until impostors exist.
const COUNTER_SMALL: u32 = 26u;

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

fn sphere_in_frustum(centre: vec3<f32>, radius: f32) -> bool {
    for (var i = 0u; i < 5u; i = i + 1u) {
        let plane = frame.frustum[i];
        if (dot(plane.xyz, centre) + plane.w <= -radius) {
            return false;
        }
    }
    return true;
}

struct Footprint {
    lo: vec2<f32>,
    hi: vec2<f32>,
    nearest: f32,
    in_front: bool,
}

fn cover(footprint: ptr<function, Footprint>, corner: vec3<f32>) {
    let clip = frame.clip_from_relative * vec4<f32>(corner, 1.0);
    if (clip.w <= 0.0) {
        (*footprint).in_front = false;
        return;
    }
    let ndc = clip.xyz / clip.w;
    (*footprint).lo = min((*footprint).lo, ndc.xy);
    (*footprint).hi = max((*footprint).hi, ndc.xy);
    (*footprint).nearest = max((*footprint).nearest, ndc.z);
}

/// The pyramid halves each level and gives an odd last row or column to its last texel, as the
/// terrain cull reads it.
fn farthest_under(lo: vec2<f32>, hi: vec2<f32>) -> f32 {
    let uv_lo = vec2<f32>(lo.x, -hi.y) * 0.5 + 0.5;
    let uv_hi = vec2<f32>(hi.x, -lo.y) * 0.5 + 0.5;
    let pixels = vec2<u32>(frame.viewport);
    let base = textureDimensions(hiz, 0);
    let b_lo = min(min(vec2<u32>(uv_lo * frame.viewport), pixels - 1u) >> vec2<u32>(1u), base - 1u);
    let b_hi = min(min(vec2<u32>(uv_hi * frame.viewport), pixels - 1u) >> vec2<u32>(1u), base - 1u);
    let extent = b_hi - b_lo + 1u;
    let level = min(u32(ceil(log2(f32(max(extent.x, extent.y))))), frame.hiz_levels - 1u);
    let last = textureDimensions(hiz, level) - vec2<u32>(1u);
    let t_lo = min(b_lo >> vec2<u32>(level), last);
    let t_hi = min(b_hi >> vec2<u32>(level), last);
    var farthest = 1.0;
    for (var y = t_lo.y; y <= t_hi.y; y = y + 1u) {
        for (var x = t_lo.x; x <= t_hi.x; x = x + 1u) {
            farthest = min(farthest, textureLoad(hiz, vec2<u32>(x, y), level).r);
        }
    }
    return farthest;
}

fn sphere_hidden(centre: vec3<f32>, radius: f32) -> bool {
    if (frame.hiz_levels == 0u) {
        return false;
    }
    var footprint = Footprint(vec2<f32>(1e30), vec2<f32>(-1e30), 0.0, true);
    for (var corner = 0u; corner < 8u; corner = corner + 1u) {
        let offset = vec3<f32>(
            select(-radius, radius, (corner & 1u) != 0u),
            select(-radius, radius, (corner & 2u) != 0u),
            select(-radius, radius, (corner & 4u) != 0u),
        );
        cover(&footprint, centre + offset);
    }
    if (!footprint.in_front) {
        return false;
    }
    if (any(footprint.lo < vec2<f32>(-1.0)) || any(footprint.hi > vec2<f32>(1.0))) {
        return false;
    }
    return footprint.nearest < farthest_under(footprint.lo, footprint.hi);
}

fn relative_block(bx: i32, by: i32, bz: i32) -> vec3<f32> {
    return vec3<f32>(vec3<i32>(bx, by, bz) - frame.camera_section * 16);
}

/// One lane for each slot. A visible slot reserves its bones and goes to the visible list of
/// its definition. Until impostors exist, a small mob also takes a full rig.
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
        }
        if (local < 16u) {
            atomicStore(&group_visible[local], 0u);
        }
        workgroupBarrier();
        let slot = first + local;
        var keep = false;
        var definition = 0u;
        var bone_count = 0u;
        if (slot < frame.slot_count) {
            let record = culls[slot];
            let flags = record.radius_flags >> 16u;
            if ((flags & CULL_VALID) != 0u) {
                let radius = unpack2x16float(record.radius_flags & 0xffffu).x + 0.8660254;
                let centre = relative_block(record.bx, record.by, record.bz) + vec3<f32>(0.5);
                keep = sphere_in_frustum(centre, radius) && !sphere_hidden(centre, radius);
                let distance = max(length(centre - frame.camera_offset), 1e-3);
                let height = 2.0 * radius * frame.projection_scale / distance;
                if (keep && height < frame.height_threshold && (flags & CULL_FULL_RIG_ONLY) == 0u) {
                    atomicAdd(&counters[COUNTER_SMALL], 1u);
                }
            }
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
            }
        }
        var bone_offset = 0u;
        var visible_offset = 0u;
        if (keep) {
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
        if (keep) {
            let first_bone = group_bones_base + bone_offset;
            let index = group_visible_base[definition] + visible_offset;
            let fits = first_bone + bone_count <= frame.bone_capacity;
            if (index < frame.visible_capacity) {
                visible[definition * frame.visible_capacity + index] =
                    select(vec2<u32>(NO_SLOT), vec2<u32>(slot, first_bone), fits);
            }
            if (!fits || index >= frame.visible_capacity) {
                atomicAdd(&counters[COUNTER_DROPPED], 1u);
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
        indirect[draw] = count * 6u;
        indirect[draw + 1u] = 1u;
        indirect[draw + 2u] = layer_array * frame.face_capacity * 6u;
        indirect[draw + 3u] = 0u;
    }
}
