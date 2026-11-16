// Generated from `shader_layout.rs`. `cargo nextest run -p mcrs_minecraft_render`
// checks it; `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_render` rewrites it.

@group(2) @binding(6) var sky_image: texture_2d<f32>;
@group(2) @binding(7) var cloud_image: texture_2d<f32>;
@group(2) @binding(8) var cloud_depth: texture_2d<f32>;
@group(2) @binding(9) var cloud_nearest: sampler;
