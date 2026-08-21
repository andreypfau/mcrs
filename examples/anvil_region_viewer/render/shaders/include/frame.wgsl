#define_import_path anvil_region_viewer::frame

#import bevy_render::view::View
#import bevy_render::globals::Globals

struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    cave_base: u32,
    wireframe: u32,
    overhang: f32,
    animated_from: u32,
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    face_origin: u32,
}

struct Sky {
    sky_light: vec4<f32>,
    block_light: vec4<f32>,
    ambient: vec4<f32>,
    disc: vec4<f32>,
    sunrise: vec4<f32>,
    angles: vec4<f32>,
    moon: vec4<f32>,
    fog: vec4<f32>,
    cloud_color: vec4<f32>,
    cloud: vec4<f32>,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<uniform> globals: Globals;
@group(0) @binding(3) var<uniform> sky: Sky;

fn region_origin() -> vec3<f32> {
    return vec3<f32>(f32(params.origin_x), f32(params.origin_y), f32(params.origin_z));
}
