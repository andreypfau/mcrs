#define_import_path mcrs_minecraft_client::terrain_bindings

#import mcrs_minecraft_client::section::SectionDesc

struct AnimationFrame {
    layer: u32,
    next: u32,
    blend: f32,
    pad: u32,
};

@group(1) @binding(0) var<storage, read> quads: array<u32>;
@group(1) @binding(1) var<storage, read> vertices: array<u32>;
@group(1) @binding(2) var<storage, read> visible: array<vec2<u32>>;
@group(1) @binding(3) var atlas0: texture_2d_array<f32>;
@group(1) @binding(4) var atlas1: texture_2d_array<f32>;
@group(1) @binding(5) var atlas2: texture_2d_array<f32>;
@group(1) @binding(6) var atlas3: texture_2d_array<f32>;
@group(1) @binding(7) var atlas_sampler: sampler;
@group(1) @binding(8) var tints: texture_2d_array<f32>;
@group(1) @binding(9) var tint_sampler: sampler;
@group(1) @binding(10) var<storage, read> animations: array<AnimationFrame>;
@group(1) @binding(11) var<storage, read> faces: array<u32>;
@group(1) @binding(12) var<storage, read> sections: array<SectionDesc>;
@group(1) @binding(13) var lightmap_levels: texture_2d<f32>;

fn quad_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return extractBits(quads[base + word], shift, bits);
}

fn model_field(base: u32, word: u32, shift: u32, bits: u32) -> u32 {
    return extractBits(vertices[base + word], shift, bits);
}
