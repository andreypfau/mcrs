import mcrs_minecraft_render::shaders::include::reconstruct::Reconstruction;

@group(1) @binding(0) var albedo_ao: texture_2d<f32>;
@group(1) @binding(1) var normal_motion: texture_2d<f32>;
@group(1) @binding(2) var light: texture_2d<f32>;
@group(1) @binding(3) var depth: texture_2d<f32>;
@group(1) @binding(4) var<uniform> reconstruction: Reconstruction;
@group(1) @binding(5) var nearest: sampler;
