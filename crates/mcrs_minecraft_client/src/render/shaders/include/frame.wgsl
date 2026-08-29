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
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    visible_limit: u32,
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
