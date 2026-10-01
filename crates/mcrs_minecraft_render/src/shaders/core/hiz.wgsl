@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var level_out: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var level_in: texture_2d<f32>;

/// Reverse-Z: the farthest surface in a footprint is its smallest depth, and a box has to be
/// nearer than that everywhere in its footprint to show at all. A source of odd size gives its
/// last row or column to the last texel of the level below it, so no surface is ever lost.
fn footprint(id: vec2<u32>, size: vec2<u32>, source: vec2<u32>) -> vec4<u32> {
    let lo = id * 2u;
    let hi = select(lo + 2u, source, id == size - 1u);
    return vec4<u32>(lo, hi);
}

@compute @workgroup_size(8, 8)
fn hiz_from_depth(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(level_out);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let span = footprint(id.xy, size, textureDimensions(depth));
    var farthest = 1.0;
    for (var y = span.y; y < span.w; y = y + 1u) {
        for (var x = span.x; x < span.z; x = x + 1u) {
            farthest = min(farthest, textureLoad(depth, vec2<u32>(x, y), 0));
        }
    }
    textureStore(level_out, id.xy, vec4<f32>(farthest, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8)
fn hiz_down(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(level_out);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let span = footprint(id.xy, size, textureDimensions(level_in));
    var farthest = 1.0;
    for (var y = span.y; y < span.w; y = y + 1u) {
        for (var x = span.x; x < span.z; x = x + 1u) {
            farthest = min(farthest, textureLoad(level_in, vec2<u32>(x, y), 0).r);
        }
    }
    textureStore(level_out, id.xy, vec4<f32>(farthest, 0.0, 0.0, 0.0));
}
