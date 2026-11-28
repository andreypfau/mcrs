import mcrs_minecraft_render::shaders::include::fields::{
    FLUID_INSET,
    MODEL_OVERHANG, MODEL_STEPS,
    MODEL_X_SHIFT, MODEL_X_BITS, MODEL_Y_SHIFT, MODEL_Y_BITS, MODEL_Z_SHIFT, MODEL_Z_BITS,
    QUAD_DROP_WORD, QUAD_DROP_SHIFT, QUAD_DROP_BITS,
    QUAD_FACE_WORD, QUAD_FACE_SHIFT, QUAD_FACE_BITS,
    QUAD_FLUID_WORD, QUAD_FLUID_SHIFT, QUAD_FLUID_BITS,
    QUAD_H_WORD, QUAD_H_SHIFT, QUAD_H_BITS,
    QUAD_W_WORD, QUAD_W_SHIFT, QUAD_W_BITS,
    QUAD_WORDS,
    QUAD_X_WORD, QUAD_X_SHIFT, QUAD_X_BITS,
    QUAD_Y_WORD, QUAD_Y_SHIFT, QUAD_Y_BITS,
    QUAD_Z_WORD, QUAD_Z_SHIFT, QUAD_Z_BITS,
};
import mcrs_minecraft_render::shaders::include::words::bits;

fn degenerate() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

const VERTICES_PER_QUAD: u32 = 6u;
const CORNERS_PER_QUAD: u32 = 4u;
const MODEL_WORDS_PER_VERTEX: u32 = 3u;
const GREEDY_EDGE_OVERLAP: f32 = 1.0 / 128.0;

fn model_quad_uv(p: vec2<f32>, uv0: vec2<f32>, uv1: vec2<f32>, uv2: vec2<f32>, uv3: vec2<f32>) -> vec2<f32> {
    return uv0 + (uv3 - uv0) * p.x + (uv1 - uv0) * p.y
        + (uv0 + uv2 - uv1 - uv3) * min(p.x, p.y);
}

fn corner_index(vertex: u32) -> u32 {
    switch vertex % VERTICES_PER_QUAD {
        case 0u: { return 1u; }
        case 1u, 4u: { return 2u; }
        case 2u, 3u: { return 0u; }
        default: { return 3u; }
    }
}

fn quad_of(vertex: u32) -> u32 {
    return vertex / VERTICES_PER_QUAD;
}

fn corner_uv(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(0.0, 1.0); }
        case 2u: { return vec2<f32>(1.0, 1.0); }
        default: { return vec2<f32>(1.0, 0.0); }
    }
}

fn face_u_dir(face: u32) -> vec3<f32> {
    switch face {
        case 0u, 1u, 3u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 2u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, 0.0, -1.0); }
    }
}

fn face_v_dir(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 1u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, -1.0, 0.0); }
    }
}

fn face_normal(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(0.0, -1.0, 0.0); }
        case 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 3u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 4u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 5u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 6u: { return vec3<f32>(1.0, 0.0, 1.0); }
        case 7u: { return vec3<f32>(1.0, 0.0, -1.0); }
        case 8u: { return vec3<f32>(-1.0, 0.0, 1.0); }
        default: { return vec3<f32>(-1.0, 0.0, -1.0); }
    }
}

struct GreedyCorner {
    world: vec3<f32>,
    quad_uv: vec2<f32>,
}

fn greedy_corner(words: array<u32, QUAD_WORDS>, origin: vec3<f32>, scale: f32, corner: u32, overlap: f32) -> GreedyCorner {
    let local = vec3<f32>(
        f32(bits(words[QUAD_X_WORD], QUAD_X_SHIFT, QUAD_X_BITS)),
        f32(bits(words[QUAD_Y_WORD], QUAD_Y_SHIFT, QUAD_Y_BITS)),
        f32(bits(words[QUAD_Z_WORD], QUAD_Z_SHIFT, QUAD_Z_BITS)),
    );
    let face = bits(words[QUAD_FACE_WORD], QUAD_FACE_SHIFT, QUAD_FACE_BITS);
    let size = vec2<f32>(
        f32(bits(words[QUAD_W_WORD], QUAD_W_SHIFT, QUAD_W_BITS) + 1u),
        f32(bits(words[QUAD_H_WORD], QUAD_H_SHIFT, QUAD_H_BITS) + 1u),
    );

    let drop = f32(bits(words[QUAD_DROP_WORD], QUAD_DROP_SHIFT, QUAD_DROP_BITS)) / MODEL_STEPS;
    var quad_uv = corner_uv(corner);
    if (face >= 2u) {
        quad_uv.y = max(quad_uv.y, drop / size.y);
    }
    let fluid = bits(words[QUAD_FLUID_WORD], QUAD_FLUID_SHIFT, QUAD_FLUID_BITS);
    let margin = select(0.0, overlap, fluid == 0u && drop == 0.0);
    let c = quad_uv * size + (quad_uv * 2.0 - vec2<f32>(1.0)) * margin;
    var world = origin + (local + face_u_dir(face) * c.x + face_v_dir(face) * c.y) * scale;
    if (face == 1u) {
        world.y -= drop;
    }
    world -= face_normal(face) * (FLUID_INSET * f32(fluid));
    return GreedyCorner(world, quad_uv);
}

fn model_position(x: u32, y: u32, z: u32) -> vec3<f32> {
    return vec3<f32>(
        f32(bits(x, MODEL_X_SHIFT, MODEL_X_BITS)),
        f32(bits(y, MODEL_Y_SHIFT, MODEL_Y_BITS)),
        f32(bits(z, MODEL_Z_SHIFT, MODEL_Z_BITS)),
    ) / MODEL_STEPS - MODEL_OVERHANG;
}

fn expanded_quad_uv(u: vec3<f32>, v: vec3<f32>, quad_uv: vec2<f32>, overlap: f32) -> vec2<f32> {
    let margin = vec2<f32>(
        overlap / max(length(u), 1e-4),
        overlap / max(length(v), 1e-4),
    );
    return quad_uv + (quad_uv * 2.0 - vec2<f32>(1.0)) * margin;
}
