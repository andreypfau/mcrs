#define_import_path mcrs_minecraft_client::frame

struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    overhang: f32,
    counter: u32,
}

struct Camera {
    clip_from_relative: mat4x4<f32>,
    frustum: array<vec4<f32>, 5>,
    section: vec3<i32>,
    offset: vec3<f32>,
    tint_origin: vec2<f32>,
    tint_scale: vec2<f32>,
    animated_from: u32,
    hiz_levels: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<uniform> camera: Camera;
