import bevy_render::view::View;
import mcrs_minecraft_render::shaders::include::sky::Sky;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> sky: Sky;

@group(1) @binding(0) var celestials: texture_2d_array<f32>;
@group(1) @binding(1) var celestial_sampler: sampler;
@group(1) @binding(2) var clouds: texture_2d<f32>;
