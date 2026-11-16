// Generated from `shader_layout.rs`. `cargo nextest run -p mcrs_minecraft_render`
// checks it; `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_render` rewrites it.

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
    occluder: vec4<f32>,
}

