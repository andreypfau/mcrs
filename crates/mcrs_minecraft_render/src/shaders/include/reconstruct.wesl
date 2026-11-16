// Generated from `shader_layout.rs`. `cargo nextest run -p mcrs_minecraft_render`
// checks it; `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_render` rewrites it.

struct Reconstruction {
    relative_from_clip: mat4x4<f32>,
    viewport: vec2<f32>,
}

