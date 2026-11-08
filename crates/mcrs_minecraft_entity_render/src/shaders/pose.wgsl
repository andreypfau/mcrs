// The pose pass of one client entity definition. The generated `pose_offsets` of the definition
// comes before this part: it adds the channels of the play list to the palette offsets.

const PALETTE: u32 = #{PALETTE}u;
const DEFINITION: u32 = #{DEFINITION}u;
const DEATH_FLIP: f32 = #{DEATH_FLIP};
const NO_INPUT: u32 = 0xffffffffu;

var<private> rotation: array<vec3<f32>, PALETTE>;
var<private> position: array<vec3<f32>, PALETTE>;
var<private> scale: array<vec3<f32>, PALETTE>;

struct PoseContext {
    input: u32,
    clock: f32,
    partial_tick: f32,
    walk_position: f32,
    walk_speed: f32,
    pitch: f32,
    body_yaw: f32,
    head_yaw: f32,
    relative_head_yaw: f32,
    life_time: f32,
    death_time: f32,
    baby: f32,
    alive: f32,
}

fn input_value(ctx: PoseContext, index: u32) -> f32 {
    if (ctx.input == NO_INPUT) {
        return 0.0;
    }
    return inputs[ctx.input].values[index];
}

fn clip_start(ctx: PoseContext, index: u32) -> f32 {
    if (ctx.input == NO_INPUT) {
        return 0.0;
    }
    return bitcast<f32>(inputs[ctx.input].clips[index].y);
}

fn catmull_rom(p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>, p3: vec3<f32>, alpha: f32) -> vec3<f32> {
    let alpha2 = alpha * alpha;
    let alpha3 = alpha2 * alpha;
    return 0.5 * (2.0 * p1
        + (p2 - p0) * alpha
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * alpha2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * alpha3);
}

fn write_rows(at: u32, linear: mat3x3<f32>, translation: vec3<f32>) {
    bone_matrices[at * 3u] = vec4<f32>(linear[0].x, linear[1].x, linear[2].x, translation.x);
    bone_matrices[at * 3u + 1u] = vec4<f32>(linear[0].y, linear[1].y, linear[2].y, translation.y);
    bone_matrices[at * 3u + 2u] = vec4<f32>(linear[0].z, linear[1].z, linear[2].z, translation.z);
}

fn read_linear(at: u32) -> mat3x3<f32> {
    let r0 = bone_matrices[at * 3u];
    let r1 = bone_matrices[at * 3u + 1u];
    let r2 = bone_matrices[at * 3u + 2u];
    return mat3x3<f32>(vec3<f32>(r0.x, r1.x, r2.x), vec3<f32>(r0.y, r1.y, r2.y), vec3<f32>(r0.z, r1.z, r2.z));
}

fn read_translation(at: u32) -> vec3<f32> {
    return vec3<f32>(bone_matrices[at * 3u].w, bone_matrices[at * 3u + 1u].w, bone_matrices[at * 3u + 2u].w);
}

/// Composes the bones of one model with the offsets of the palette, from `first`.
fn write_model_bones(model: ModelRecord, first: u32) {
    for (var index = 0u; index < model.bone_count; index = index + 1u) {
        let bone = bones[model.first_bone + index];
        let palette = bone.parent_palette >> 16u;
        var turn = vec3<f32>(bone.rest_x, bone.rest_y, bone.rest_z);
        var offset = vec3<f32>(0.0);
        var size = vec3<f32>(bone.scale_x, bone.scale_y, bone.scale_z);
        if (palette < PALETTE) {
            let animated = rotation[palette];
            turn = turn + vec3<f32>(-animated.x, -animated.y, animated.z);
            let moved = position[palette];
            offset = vec3<f32>(-moved.x, moved.y, moved.z);
            size = size * scale[palette];
        }
        let pivot = vec3<f32>(bone.pivot_x, bone.pivot_y, bone.pivot_z);
        let turned = rotation_z(turn.z) * rotation_y(turn.y) * rotation_x(turn.x);
        let linear = turned * mat3x3<f32>(
            vec3<f32>(size.x, 0.0, 0.0),
            vec3<f32>(0.0, size.y, 0.0),
            vec3<f32>(0.0, 0.0, size.z),
        );
        let translation = offset + pivot - linear * pivot;
        let parent = bone.parent_palette & 0xffffu;
        if (parent == NO_PARENT) {
            write_rows(first + index, linear, translation);
        } else {
            let parent_linear = read_linear(first + parent);
            let parent_translation = read_translation(first + parent);
            write_rows(first + index, parent_linear * linear, parent_linear * translation + parent_translation);
        }
    }
}

@compute @workgroup_size(64)
fn pose(@builtin(global_invocation_id) id: vec3<u32>) {
    let count = min(atomicLoad(&counters[COUNTER_VISIBLE + DEFINITION]), frame.visible_capacity);
    if (id.x >= count) {
        return;
    }
    let index = DEFINITION * frame.visible_capacity + id.x;
    let entry = visible[index];
    if (entry.x == NO_SLOT) {
        return;
    }
    let slot = entry.x;
    let state = states[slot];
    let current = ticks[slot];
    let interpolates = tick_of_record[slot] == frame.tick;
    var previous = current;
    if (interpolates) {
        previous = previous_ticks[slot];
    }
    let pt = frame.partial_tick;

    var age = f32(i32(frame.tick) - state.clock_offset);
    var whole_age = i32(frame.tick) - state.clock_offset;
    if ((state.flags & STATE_CLOCK_STOPPED) != 0u) {
        age = f32(state.clock_offset);
        whole_age = state.clock_offset;
    }
    let clock = age + pt;
    var death_time = 0.0;
    if ((state.flags & STATE_DYING) != 0u) {
        death_time = max(clock - state.death_start, 0.0);
    }
    let body_yaw = wrap_degrees(alerp(pt, turn_degrees(previous.fraction_z_body >> 16u), turn_degrees(current.fraction_z_body >> 16u)));
    let head_yaw = wrap_degrees(alerp(pt, turn_degrees(previous.head_pitch), turn_degrees(current.head_pitch)));
    let pitch = mix(record_pitch(previous), record_pitch(current), pt);
    let walk_position = mix(previous.walk_position, current.walk_position, pt);
    let walk_speed = min(mix(previous.walk_speed, current.walk_speed, pt), 1.0);

    let ctx = PoseContext(
        state.input,
        clock,
        pt,
        walk_position,
        walk_speed,
        pitch,
        body_yaw,
        head_yaw,
        wrap_degrees(head_yaw - body_yaw),
        clock / 20.0,
        death_time,
        select(0.0, 1.0, (state.flags & STATE_BABY) != 0u),
        select(1.0, 0.0, (state.flags & STATE_DYING) != 0u),
    );
    for (var palette = 0u; palette < PALETTE; palette = palette + 1u) {
        rotation[palette] = vec3<f32>(0.0);
        position[palette] = vec3<f32>(0.0);
        scale[palette] = vec3<f32>(1.0);
    }
    pose_offsets(ctx);

    var first = entry.y;
    let mask = layer_mask(state);
    for (var layer = 0u; layer < 4u; layer = layer + 1u) {
        if ((mask & (1u << layer)) != 0u) {
            let model = models[layer_word(state, layer) & 0xffffu];
            write_model_bones(model, first);
            first = first + model.bone_count;
        }
    }

    var tilt = mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0));
    if (death_time > 0.0) {
        let fall = min(sqrt(max((death_time - 1.0) / 20.0 * 1.6, 0.0)), 1.0);
        tilt = rotation_z(fall * DEATH_FLIP);
    }
    let entity_scale = unpack2x16float(state.light_scale >> 16u).x;
    let linear = rotation_y(180.0 - body_yaw) * tilt * (entity_scale / 16.0);
    let placed = mix(record_position(previous), record_position(current), pt);
    let translation = placed + linear * vec3<f32>(0.0, 0.016, 0.0);
    var instance: Instance;
    instance.row0 = vec4<f32>(linear[0].x, linear[1].x, linear[2].x, translation.x);
    instance.row1 = vec4<f32>(linear[0].y, linear[1].y, linear[2].y, translation.y);
    instance.row2 = vec4<f32>(linear[0].z, linear[1].z, linear[2].z, translation.z);
    instance.overlay = entity_overlay(state, whole_age, death_time > 0.0);
    instance.light = entity_light(state);
    instances[index] = instance;
}
