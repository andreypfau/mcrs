// Generated from `shader_layout.rs`. `cargo nextest run -p mcrs_minecraft_render`
// checks it; `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_render` rewrites it.

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var level_out: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var level_in: texture_2d<f32>;
