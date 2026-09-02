#define_import_path mcrs_minecraft_client::frame

#import bevy_render::view::View
#import bevy_render::globals::Globals

struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    wireframe: u32,
    overhang: f32,
    animated_from: u32,
    padding: u32,
}

struct Camera {
    clip_from_relative: mat4x4<f32>,
    frustum: array<vec4<f32>, 5>,
    section: vec3<i32>,
    offset: vec3<f32>,
    tint_origin: vec2<f32>,
    tint_span: vec2<f32>,
}

struct Sky {
    disc: vec4<f32>,
    sunrise: vec4<f32>,
    angles: vec4<f32>,
    moon: vec4<f32>,
    fog: vec4<f32>,
    cloud_color: vec4<f32>,
    cloud: vec4<f32>,
    sky_light: vec4<f32>,
    block_light: vec4<f32>,
    ambient: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<uniform> globals: Globals;
@group(0) @binding(3) var<uniform> sky: Sky;
@group(0) @binding(4) var<uniform> camera: Camera;
