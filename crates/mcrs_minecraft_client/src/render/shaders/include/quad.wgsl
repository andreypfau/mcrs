#define_import_path mcrs_minecraft_client::quad

fn degenerate() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

const VERTICES_PER_QUAD: u32 = 4u;

/// The index list names each vertex as four times its quad plus its corner.
fn corner_index(vertex: u32) -> u32 {
    return vertex % VERTICES_PER_QUAD;
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
