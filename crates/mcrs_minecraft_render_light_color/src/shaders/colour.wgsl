struct Job {
    words: u32,
    lane_base: u32,
    atlas: array<u32, 3>,
    slots: array<u32, 27>,
    lane_of: array<u32, 64>,
    colour: array<u32, 64>,
}

const EAST: u32 = 1u;
const UP: u32 = 2u;
const SOUTH: u32 = 4u;
const BRICK: u32 = 4096u;
const SIDE: u32 = 46u;
const LAYER: u32 = SIDE * SIDE;
const CELLS: u32 = LAYER * SIDE;
const OUTPUT: u32 = 18u;
const OUTPUT_FIRST: u32 = 14u;
const SLOT_UNLOADED: u32 = 0xffffffffu;
const SLOT_ABOVE: u32 = 0xfffffffeu;

@group(0) @binding(0) var<storage, read> jobs: array<Job>;
@group(0) @binding(1) var<storage, read> pool: array<u32>;
@group(0) @binding(2) var<storage, read_write> cost: array<u32>;
@group(0) @binding(3) var<storage, read> lanes_in: array<u32>;
@group(0) @binding(4) var<storage, read_write> lanes_out: array<u32>;
@group(0) @binding(5) var atlas: texture_storage_3d<rgba8unorm, write>;

fn bytes_of(word: u32) -> vec4<u32> {
    return (vec4<u32>(word) >> vec4<u32>(0u, 8u, 16u, 24u)) & vec4<u32>(0xffu);
}

fn word_of(bytes: vec4<u32>) -> u32 {
    return bytes.x | (bytes.y << 8u) | (bytes.z << 16u) | (bytes.w << 24u);
}

// The region starts one cell into the neighbour below on every axis.
fn packed_cell(job: u32, at: vec3<u32>) -> u32 {
    let shifted = at + 1u;
    let d = shifted / 16u;
    let slot = jobs[job].slots[d.x + 3u * (d.z + 3u * d.y)];
    if slot == SLOT_UNLOADED {
        return 15u;
    }
    if slot == SLOT_ABOVE {
        return 1u;
    }
    let l = shifted % 16u;
    return pool[slot * BRICK + (l.x | (l.z << 4u) | (l.y << 8u))];
}

fn lane_of(job: u32, t: u32) -> u32 {
    return (jobs[job].lane_of[t / 4u] >> (8u * (t % 4u))) & 0xffu;
}

@compute @workgroup_size(64)
fn gather(@builtin(global_invocation_id) id: vec3<u32>) {
    let job = id.y;
    let cell = id.x;
    if cell >= CELLS {
        return;
    }
    let packed = packed_cell(job, vec3<u32>(cell % SIDE, cell / LAYER, cell / SIDE % SIDE));
    cost[job * CELLS + cell] = packed & 0x7fu;
    let emission = (packed >> 8u) & 15u;
    let lane = lane_of(job, (packed >> 16u) & 0xffu);
    let words = jobs[job].words;
    let base = jobs[job].lane_base + cell * words;
    for (var word = 0u; word < words; word++) {
        var seed = 0u;
        if emission > 0u && lane / 4u == word {
            seed = emission << (8u * (lane % 4u));
        }
        lanes_out[base + word] = seed;
    }
}

@compute @workgroup_size(64)
fn waves(@builtin(global_invocation_id) id: vec3<u32>) {
    let job = id.y;
    let words = jobs[job].words;
    let i = id.x;
    if i >= CELLS * words {
        return;
    }
    let cell = i / words;
    let x = cell % SIDE;
    let z = cell / SIDE % SIDE;
    let y = cell / LAYER;
    let costs = job * CELLS;
    let lanes = jobs[job].lane_base + i % words;
    let veto = cost[costs + cell] >> 4u;

    var incoming = vec4<u32>(0u);
    if x > 0u && ((cost[costs + cell - 1u] >> 4u) & EAST) == 0u {
        incoming = max(incoming, bytes_of(lanes_in[lanes + (cell - 1u) * words]));
    }
    if x + 1u < SIDE && (veto & EAST) == 0u {
        incoming = max(incoming, bytes_of(lanes_in[lanes + (cell + 1u) * words]));
    }
    if z > 0u && ((cost[costs + cell - SIDE] >> 4u) & SOUTH) == 0u {
        incoming = max(incoming, bytes_of(lanes_in[lanes + (cell - SIDE) * words]));
    }
    if z + 1u < SIDE && (veto & SOUTH) == 0u {
        incoming = max(incoming, bytes_of(lanes_in[lanes + (cell + SIDE) * words]));
    }
    if y > 0u && ((cost[costs + cell - LAYER] >> 4u) & UP) == 0u {
        incoming = max(incoming, bytes_of(lanes_in[lanes + (cell - LAYER) * words]));
    }
    if y + 1u < SIDE && (veto & UP) == 0u {
        incoming = max(incoming, bytes_of(lanes_in[lanes + (cell + LAYER) * words]));
    }

    let entry = vec4<u32>(cost[costs + cell] & 15u);
    let arriving = max(incoming, entry) - entry;
    let own = bytes_of(lanes_in[lanes + cell * words]);
    lanes_out[lanes + cell * words] = word_of(max(own, arriving));
}

fn light_weight(level: u32) -> f32 {
    let f = f32(level) / 15.0;
    return f / (4.0 - 3.0 * f);
}

@compute @workgroup_size(64)
fn resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    let job = id.y;
    let cell = id.x;
    if cell >= OUTPUT * OUTPUT * OUTPUT {
        return;
    }
    let out = vec3<u32>(cell % OUTPUT, cell / (OUTPUT * OUTPUT), cell / OUTPUT % OUTPUT);
    let at = out + OUTPUT_FIRST;
    let words = jobs[job].words;
    let base = jobs[job].lane_base + (at.x + SIDE * (at.z + SIDE * at.y)) * words;

    var rgb = vec3<f32>(0.0);
    var total = 0.0;
    var default_weight = 0.0;
    for (var word = 0u; word < words; word++) {
        let levels = bytes_of(lanes_in[base + word]);
        for (var k = 0u; k < 4u; k++) {
            let weight = light_weight(levels[k]);
            total += weight;
            let colour = jobs[job].colour[word * 4u + k];
            if (colour >> 24u) != 0u {
                rgb += vec3<f32>(bytes_of(colour).xyz) * weight;
            } else {
                default_weight += weight;
            }
        }
    }

    // Half up, as the CPU rounds these positive values; the WGSL built-in rounds half to even.
    var texel = vec4<f32>(0.0, 0.0, 0.0, 255.0);
    if total != 0.0 {
        texel = vec4<f32>(floor(rgb / total + 0.5), floor(default_weight * 255.0 / total + 0.5));
    }
    let origin = vec3<u32>(jobs[job].atlas[0], jobs[job].atlas[1], jobs[job].atlas[2]);
    textureStore(atlas, origin + out, texel / 255.0);
}
