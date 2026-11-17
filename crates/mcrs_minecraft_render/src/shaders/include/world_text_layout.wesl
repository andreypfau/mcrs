struct TextFrame {
    clip_from_relative: mat4x4<f32>,
    section: vec3<i32>,
}

@group(0) @binding(0) var<uniform> frame: TextFrame;

@group(1) @binding(0) var font_page: texture_2d<f32>;
@group(1) @binding(1) var font_sampler: sampler;
@group(1) @binding(2) var lightmap_levels: texture_2d<f32>;
@group(1) @binding(3) var lightmap_sampler: sampler;
