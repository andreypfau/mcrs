#define_import_path anvil_region_viewer::quad

/// Both geometry kinds draw a quad as a four vertex triangle strip, so the strip order and
/// the corner order around the quad are not the same walk.
fn corner_index(vertex: u32) -> u32 {
    switch vertex {
        case 0u: { return 1u; }
        case 1u: { return 2u; }
        case 2u: { return 0u; }
        default: { return 3u; }
    }
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

fn face_normal(u_dir: vec3<f32>, v_dir: vec3<f32>) -> vec3<f32> {
    return -cross(u_dir, v_dir);
}
