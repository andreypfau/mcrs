struct Params {
    size: u32,
    words: u32,
}

const EAST: u32 = 1u;
const UP: u32 = 2u;
const SOUTH: u32 = 4u;

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> cost: array<u32>;
@group(0) @binding(2) var<storage, read> from_lanes: array<u32>;
@group(0) @binding(3) var<storage, read_write> to_lanes: array<u32>;

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
