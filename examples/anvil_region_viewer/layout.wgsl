#define_import_path anvil_region_viewer::layout

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

const SECTION_SIZE: f32 = 16.0;

const CULLED: u32 = 0xffffffffu;

const LOCAL_X_WORD: u32 = 0u;
const LOCAL_X_SHIFT: u32 = 0u;
const LOCAL_X_BITS: u32 = 4u;
const LOCAL_Y_WORD: u32 = 0u;
const LOCAL_Y_SHIFT: u32 = 4u;
const LOCAL_Y_BITS: u32 = 3u;
const LOCAL_Z_WORD: u32 = 0u;
const LOCAL_Z_SHIFT: u32 = 7u;
const LOCAL_Z_BITS: u32 = 4u;

fn section_origin(section: u32, region: vec3<f32>) -> vec3<f32> {
    let local = vec3<f32>(
        f32(extractBits(section, LOCAL_X_SHIFT, LOCAL_X_BITS)),
        f32(extractBits(section, LOCAL_Y_SHIFT, LOCAL_Y_BITS)),
        f32(extractBits(section, LOCAL_Z_SHIFT, LOCAL_Z_BITS)),
    );
    return region + local * SECTION_SIZE;
}

fn degenerate() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
