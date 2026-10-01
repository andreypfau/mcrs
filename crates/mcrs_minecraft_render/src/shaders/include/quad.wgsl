#define_import_path mcrs_minecraft_client::quad

#import mcrs_minecraft_client::fields::{
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
}

fn degenerate() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

const VERTICES_PER_QUAD: u32 = 6u;
const CORNERS_PER_QUAD: u32 = 4u;
const MODEL_WORDS_PER_VERTEX: u32 = 3u;

/// Two triangles a quad, wound over its corners as 1, 2, 0 and 0, 2, 3.
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

/// One corner of a greedy quad against the camera's section. The cull and the vertex shaders
/// both place corners here, so a quad is tested exactly where it is drawn.
fn greedy_corner(words: array<u32, QUAD_WORDS>, origin: vec3<f32>, scale: f32, corner: u32) -> GreedyCorner {
    let local = vec3<f32>(
        f32(extractBits(words[QUAD_X_WORD], QUAD_X_SHIFT, QUAD_X_BITS)),
        f32(extractBits(words[QUAD_Y_WORD], QUAD_Y_SHIFT, QUAD_Y_BITS)),
        f32(extractBits(words[QUAD_Z_WORD], QUAD_Z_SHIFT, QUAD_Z_BITS)),
    );
    let face = extractBits(words[QUAD_FACE_WORD], QUAD_FACE_SHIFT, QUAD_FACE_BITS);
    let size = vec2<f32>(
        f32(extractBits(words[QUAD_W_WORD], QUAD_W_SHIFT, QUAD_W_BITS) + 1u),
        f32(extractBits(words[QUAD_H_WORD], QUAD_H_SHIFT, QUAD_H_BITS) + 1u),
    );

    // Fluids sit below the top of their block, so the surface drops and the sides shorten.
    let drop = f32(extractBits(words[QUAD_DROP_WORD], QUAD_DROP_SHIFT, QUAD_DROP_BITS)) / MODEL_STEPS;
    var quad_uv = corner_uv(corner);
    if (face >= 2u) {
        quad_uv.y = max(quad_uv.y, drop / size.y);
    }
    let c = quad_uv * size;
    var world = origin + (local + face_u_dir(face) * c.x + face_v_dir(face) * c.y) * scale;
    if (face == 1u) {
        world.y -= drop;
    }
    // A fluid face shares a plane with the block face behind it, so it is pulled in slightly.
    let fluid = extractBits(words[QUAD_FLUID_WORD], QUAD_FLUID_SHIFT, QUAD_FLUID_BITS);
    world -= face_normal(face) * (FLUID_INSET * f32(fluid));
    return GreedyCorner(world, quad_uv);
}

/// A model vertex's position in its section from the words holding its x, y and z. Positions
/// are stored in steps of a block and biased so a model may lean into its neighbours by the
/// overhang the mesher allowed for.
fn model_position(x: u32, y: u32, z: u32) -> vec3<f32> {
    return vec3<f32>(
        f32(extractBits(x, MODEL_X_SHIFT, MODEL_X_BITS)),
        f32(extractBits(y, MODEL_Y_SHIFT, MODEL_Y_BITS)),
        f32(extractBits(z, MODEL_Z_SHIFT, MODEL_Z_BITS)),
    ) / MODEL_STEPS - MODEL_OVERHANG;
}
