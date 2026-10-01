#define_import_path mcrs_minecraft_client::frame

struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    counter: u32,
    flags: u32,
}

const PARAMS_MODEL: u32 = 1u;
const PARAMS_QUAD_CULL: u32 = 2u;

struct Camera {
    clip_from_relative: mat4x4<f32>,
    frustum: array<vec4<f32>, 5>,
    section: vec3<i32>,
    offset: vec3<f32>,
    tint_origin: vec2<f32>,
    tint_scale: vec2<f32>,
    hiz_levels: u32,
    quad_cull: u32,
    viewport: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<uniform> camera: Camera;
