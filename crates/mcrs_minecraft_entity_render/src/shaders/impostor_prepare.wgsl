// The prepare pass of the impostors: one lane for each slot of the impostor list. A lane selects
// the view and writes the draw record of the impostor, so the vertex stage of the impostor draw
// only places the corners of its quad.

const TILE: u32 = #{TILE}u;
const YAWS: i32 = #{YAWS};
/// The elevations of the views in degrees, rising.
const ELEVATIONS = #{ELEVATIONS};
const ELEVATION_COUNT: u32 = #{ELEVATION_COUNT}u;
const VIEWS: u32 = #{VIEWS}u;
const NO_ENTRY: u32 = 0xffffu;
/// The projected height in pixels under which an impostor draws its whole quad with no discard.
const SMALL_HEIGHT: f32 = #{SMALL_HEIGHT};

fn layer_entry(state: StateRecord, layer: u32) -> u32 {
    if ((layer_mask(state) & (1u << layer)) == 0u) {
        return NO_ENTRY;
    }
    let word = layer_word(state, layer);
    let at = (word & 0xffffu) * VARIANTS + (word >> 16u);
    if (at >= arrayLength(&impostor_pairs)) {
        return NO_ENTRY;
    }
    return impostor_pairs[at] & 0xffffu;
}

fn layer_rect(state: StateRecord, layer: u32, view: u32) -> vec4<f32> {
    let at = (layer_word(state, layer) & 0xffffu) * VIEWS + view;
    if ((layer_mask(state) & (1u << layer)) == 0u || at >= FIRST_COVER) {
        return vec4<f32>(0.0);
    }
    return impostor_covers[at];
}

@compute @workgroup_size(64)
fn prepare(@builtin(global_invocation_id) id: vec3<u32>) {
    let count = min(atomicLoad(&counters[COUNTER_IMPOSTORS]), arrayLength(&impostors));
    if (id.x >= count) {
        return;
    }
    let slot = impostors[id.x];
    let state = states[slot];
    let current = ticks[current_tick(slot)];
    var previous = current;
    if (tick_of_record[slot] == frame.tick) {
        previous = ticks[previous_tick(slot)];
    }
    let pt = frame.partial_tick;
    let body_yaw = wrap_degrees(alerp(pt, turn_degrees(previous.fraction_z_body >> 16u), turn_degrees(current.fraction_z_body >> 16u)));
    let position = mix(record_position(previous), record_position(current), pt);
    let scale = unpack2x16float(state.light_scale >> 16u).x;
    let first = min(countTrailingZeros(layer_mask(state)), 3u);
    let level = layer_rect(state, first, 0u);
    let to_camera = frame.camera_offset - (position + vec3<f32>(0.0, 0.5 * (level.z + level.w) * scale, 0.0));
    let across = length(to_camera.xz);
    let facing = select(vec2<f32>(0.0, 1.0), to_camera.xz / across, across > 1e-4);
    let elevation = degrees(atan2(to_camera.y, across));
    var elevations = ELEVATIONS;
    var row = 0u;
    for (var index = 1u; index < ELEVATION_COUNT; index = index + 1u) {
        if (abs(elevation - elevations[index]) < abs(elevation - elevations[row])) {
            row = index;
        }
    }
    let azimuth = degrees(atan2(facing.x, facing.y)) + body_yaw;
    let step = i32(round(azimuth * f32(YAWS) / 360.0));
    let yaw = u32(((step % YAWS) + YAWS) % YAWS);
    let view = row * u32(YAWS) + yaw;
    var draw: ImpostorDraw;
    var bounds = vec4<f32>(1e30, -1e30, 1e30, -1e30);
    for (var layer = 0u; layer < 4u; layer = layer + 1u) {
        let rect = layer_rect(state, layer, view);
        draw.rects[layer] = rect;
        if ((layer_mask(state) & (1u << layer)) != 0u) {
            bounds = vec4<f32>(min(bounds.x, rect.x), max(bounds.y, rect.y), min(bounds.z, rect.z), max(bounds.w, rect.w));
        }
    }
    let whole_age = i32(frame.tick) - state.clock_offset;
    let overlay = entity_overlay(state, whole_age, false);
    let light = entity_light(state);
    draw.origin_yaw = vec4<f32>(position, body_yaw);
    draw.axes = vec4<f32>(facing.y * scale, -facing.x * scale, scale, bitcast<f32>(yaw * TILE | ((row * TILE) << 16u)));
    draw.bounds = bounds;
    draw.words = vec4<u32>(
        layer_entry(state, 0u) | (layer_entry(state, 1u) << 16u),
        layer_entry(state, 2u) | (layer_entry(state, 3u) << 16u),
        u32(light.x) | (u32(light.y) << 16u),
        select(0u, 1u, overlay.a < 1.0),
    );
    var sizes = BOX_SIZES;
    let size = sizes[min(state_definition(state), DEFINITIONS - 1u)];
    let baby = select(1.0, size.y, (state.flags & STATE_BABY) != 0u);
    let pixels = size.x * baby * scale * frame.projection_scale / max(length(to_camera), 1e-3);
    if (pixels < SMALL_HEIGHT) {
        // A small impostor draws a rectangle with the covered area of its image, about the
        // centroid of the covered texels, so its pixels cover about what the image covers.
        let rect = draw.rects[first];
        let cover = impostor_covers[FIRST_COVER + layer_entry(state, first) * VIEWS + view];
        if (cover.z > 0.0) {
            let shrink = sqrt(cover.z) * 0.5;
            let size = vec2<f32>(rect.y - rect.x, rect.w - rect.z);
            let centre = vec2<f32>(rect.x + cover.x * size.x, rect.w - cover.y * size.y);
            draw.bounds = vec4<f32>(centre.x - shrink * size.x, centre.x + shrink * size.x, centre.y - shrink * size.y, centre.y + shrink * size.y);
        }
        let at = atomicAdd(&counters[COUNTER_SMALL_IMPOSTORS], 1u);
        impostor_draws[frame.slot_count - 1u - at] = draw;
    } else {
        impostor_draws[atomicAdd(&counters[COUNTER_CUTOUT_IMPOSTORS], 1u)] = draw;
    }
}
