struct Frame {
    clip_from_relative: mat4x4<f32>,
    section: vec3<i32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

@group(1) @binding(0) var font_page: texture_2d<f32>;
@group(1) @binding(1) var font_sampler: sampler;
@group(1) @binding(2) var lightmap_levels: texture_2d<f32>;
@group(1) @binding(3) var lightmap_sampler: sampler;

struct Vertex {
    @location(0) block: vec3<i32>,
    @location(1) offset: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) light: vec2<f32>,
};

struct Fragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) light: vec2<f32>,
};

const SECTION_SIZE: i32 = 16;

@vertex
fn vertex(in: Vertex) -> Fragment {
    var out: Fragment;
    // The block is subtracted from the camera section in i32, as the terrain does.
    let relative = vec3<f32>(in.block - frame.section * SECTION_SIZE) + in.offset;
    out.clip_position = frame.clip_from_relative * vec4<f32>(relative, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.light = in.light;
    return out;
}

// The lookup of the terrain's own lightmap, which follows the time of day and the Brightness
// option: it is a copy of `lightmap` in `lighting.wgsl`, which cannot be imported next to the
// other bindings of a pass of its own.
fn lightmap(block: f32, sky: f32) -> vec3<f32> {
    let uv = clamp(
        vec2<f32>(block, sky) / 256.0 + 0.5 / 16.0,
        vec2<f32>(0.5 / 16.0),
        vec2<f32>(15.5 / 16.0),
    );
    return textureSampleLevel(lightmap_levels, lightmap_sampler, uv, 0.0).rgb;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    let texel = textureSample(font_page, font_sampler, in.uv);
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(in.color * texel.rgb * lightmap(in.light.x, in.light.y), 1.0);
}
