#define_import_path anvil_region_viewer::sky_bindings

@group(1) @binding(0) var celestials: texture_2d_array<f32>;
@group(1) @binding(1) var celestial_sampler: sampler;
@group(1) @binding(2) var clouds: texture_2d_array<f32>;
