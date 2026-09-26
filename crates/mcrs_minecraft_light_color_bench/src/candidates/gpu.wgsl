struct Params {
    size: u32,
    words: u32,
    side: u32,
    reach: u32,
    base: vec3<u32>,
    palette: array<vec4<u32>, 16>,
}

const EAST: u32 = 1u;
const UP: u32 = 2u;
const SOUTH: u32 = 4u;
const BRICK: u32 = 4096u;

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> bricks: array<u32>;
@group(0) @binding(2) var<storage, read_write> cost: array<u32>;
@group(0) @binding(3) var<storage, read> from_lanes: array<u32>;
@group(0) @binding(4) var<storage, read_write> to_lanes: array<u32>;
@group(0) @binding(5) var<storage, read_write> output: array<u32>;

fn lane_of(t: u32) -> u32 {
    return (params.palette[t / 16u][t / 4u % 4u] >> (8u * (t % 4u))) & 0xffu;
}

@compute @workgroup_size(64)
fn gather(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = params.size;
    let cell = id.x;
    if cell >= size * size * size {
        return;
    }
    let at = params.base + vec3<u32>(cell % size, cell / (size * size), cell / size % size);
    let brick = at / 16u;
    let local = at % 16u;
    let slot = brick.x + params.side * (brick.z + params.side * brick.y);
    let packed = bricks[slot * BRICK + (local.x | (local.z << 4u) | (local.y << 8u))];

    cost[cell] = packed & 0x7fu;
    let emission = (packed >> 8u) & 15u;
    let lane = lane_of((packed >> 16u) & 0xffu);
    for (var word = 0u; word < params.words; word++) {
        var seed = 0u;
        if emission > 0u && lane / 4u == word {
            seed = emission << (8u * (lane % 4u));
        }
        to_lanes[cell * params.words + word] = seed;
    }
}

fn veto(cell: u32) -> u32 {
    return cost[cell] >> 4u;
}

fn lanes_of(cell: u32, word: u32) -> vec4<u32> {
    return unpack4xU8(from_lanes[cell * params.words + word]);
}

@compute @workgroup_size(64)
fn waves(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = params.size;
    let layer = size * size;
    let i = id.x;
    if i >= layer * size * params.words {
        return;
    }
    let cell = i / params.words;
    let word = i % params.words;
    let x = cell % size;
    let z = cell / size % size;
    let y = cell / layer;

    var incoming = vec4<u32>(0u);
    if x > 0u && (veto(cell - 1u) & EAST) == 0u {
        incoming = max(incoming, lanes_of(cell - 1u, word));
    }
    if x + 1u < size && (veto(cell) & EAST) == 0u {
        incoming = max(incoming, lanes_of(cell + 1u, word));
    }
    if z > 0u && (veto(cell - size) & SOUTH) == 0u {
        incoming = max(incoming, lanes_of(cell - size, word));
    }
    if z + 1u < size && (veto(cell) & SOUTH) == 0u {
        incoming = max(incoming, lanes_of(cell + size, word));
    }
    if y > 0u && (veto(cell - layer) & UP) == 0u {
        incoming = max(incoming, lanes_of(cell - layer, word));
    }
    if y + 1u < size && (veto(cell) & UP) == 0u {
        incoming = max(incoming, lanes_of(cell + layer, word));
    }

    let entry = vec4<u32>(cost[cell] & 15u);
    let arriving = max(incoming, entry) - entry;
    to_lanes[i] = pack4xU8(max(lanes_of(cell, word), arriving));
}

@compute @workgroup_size(64)
fn cut(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = params.size;
    let out = size - 2u * params.reach;
    let i = id.x;
    if i >= out * out * out * params.words {
        return;
    }
    let cell = i / params.words;
    let word = i % params.words;
    let at = vec3<u32>(cell % out, cell / (out * out), cell / out % out) + params.reach;
    output[i] = from_lanes[(at.x + size * (at.z + size * at.y)) * params.words + word];
}
