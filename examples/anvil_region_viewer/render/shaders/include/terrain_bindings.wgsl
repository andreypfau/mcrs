#define_import_path anvil_region_viewer::terrain_bindings

struct Animation {
    base_layer: u32,
    count: u32,
    frametime: u32,
    interpolate: u32,
};

@group(1) @binding(0) var<storage, read> quads: array<u32>;
@group(1) @binding(1) var<storage, read> vertices: array<u32>;
@group(1) @binding(2) var<storage, read> visible: array<u32>;
@group(1) @binding(3) var atlas0: texture_2d_array<f32>;
@group(1) @binding(4) var atlas1: texture_2d_array<f32>;
@group(1) @binding(5) var atlas2: texture_2d_array<f32>;
@group(1) @binding(6) var atlas3: texture_2d_array<f32>;
@group(1) @binding(7) var atlas_sampler: sampler;
@group(1) @binding(8) var tints: texture_2d_array<f32>;
@group(1) @binding(9) var tint_sampler: sampler;
@group(1) @binding(10) var<storage, read> animations: array<Animation>;
@group(1) @binding(11) var<storage, read> faces: array<u32>;

fn quad_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return extractBits(quads[base + word], shift, bits);
}

fn model_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return extractBits(vertices[base + word], shift, bits);
}
