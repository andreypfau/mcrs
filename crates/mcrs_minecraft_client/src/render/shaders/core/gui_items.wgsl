struct GuiUniform {
    framebuffer: vec2<f32>,
    scale: f32,
    animated_from: u32,
    glint_offset: vec2<f32>,
    glint_alpha: f32,
    _pad: f32,
}

struct AnimationFrame {
    layer: u32,
    next: u32,
    blend: f32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> gui: GuiUniform;
@group(0) @binding(1) var atlas0: texture_2d_array<f32>;
@group(0) @binding(2) var atlas1: texture_2d_array<f32>;
@group(0) @binding(3) var atlas2: texture_2d_array<f32>;
@group(0) @binding(4) var atlas3: texture_2d_array<f32>;
@group(0) @binding(5) var atlas_sampler: sampler;
@group(0) @binding(6) var<storage, read> animations: array<AnimationFrame>;
@group(0) @binding(7) var gui_atlas: texture_2d<f32>;
@group(0) @binding(8) var glint: texture_2d<f32>;
@group(0) @binding(9) var glint_sampler: sampler;

const GUI_ATLAS_BIT: u32 = 0x80000000u;
const GLINT_BIT: u32 = 0x40000000u;
const ALPHA_CUTOUT: f32 = 0.1;
const GLINT_ROTATION: f32 = 0.17453292;
const GLINT_SCALE: f32 = 8.0;
// ponytail: vanilla projects the glint from stitched-atlas UVs, so a 16px sprite spans
// 16/atlas_px of it; the atlas width stands in as a constant and the sprite's
// position in the atlas (a per-item phase offset) is not modelled.
const GLINT_ATLAS_PX: f32 = 2048.0;

struct VertexIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) sprite: u32,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) sprite: u32,
}

@vertex
fn vs_gui(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    let pixels = in.pos.xy * gui.scale / gui.framebuffer;
    out.clip = vec4<f32>(pixels.x * 2.0 - 1.0, 1.0 - pixels.y * 2.0, (in.pos.z + 1000.0) / 2000.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.sprite = in.sprite;
    return out;
}

fn sample_atlas(array: u32, uv: vec2<f32>, layer: u32) -> vec4<f32> {
    switch array {
        case 1u: { return textureSampleLevel(atlas1, atlas_sampler, uv, layer, 0.0); }
        case 2u: { return textureSampleLevel(atlas2, atlas_sampler, uv, layer, 0.0); }
        case 3u: { return textureSampleLevel(atlas3, atlas_sampler, uv, layer, 0.0); }
        default: { return textureSampleLevel(atlas0, atlas_sampler, uv, layer, 0.0); }
    }
}

fn sprite_px(array: u32) -> vec2<f32> {
    switch array {
        case 1u: { return vec2<f32>(textureDimensions(atlas1)); }
        case 2u: { return vec2<f32>(textureDimensions(atlas2)); }
        case 3u: { return vec2<f32>(textureDimensions(atlas3)); }
        default: { return vec2<f32>(textureDimensions(atlas0)); }
    }
}

fn sample_sprite(array: u32, uv: vec2<f32>, layer: u32) -> vec4<f32> {
    if layer < gui.animated_from {
        return sample_atlas(array, uv, layer);
    }
    let frame = animations[layer - gui.animated_from];
    let color = sample_atlas(array, uv, frame.layer);
    if frame.blend == 0.0 {
        return color;
    }
    return mix(color, sample_atlas(array, uv, frame.next), frame.blend);
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// Vanilla multiplies the vertex colour and adds the glint on the stored (sRGB) texel bytes,
// so the arithmetic happens in that space and only the final value goes back to linear.
@fragment
fn fs_gui(in: VertexOut) -> @location(0) vec4<f32> {
    if (in.sprite & GUI_ATLAS_BIT) != 0u {
        let texel = textureSampleLevel(gui_atlas, atlas_sampler, in.uv, 0.0);
        let color = vec4<f32>(to_srgb(texel.rgb), texel.a) * in.color;
        return vec4<f32>(to_linear(color.rgb), color.a);
    }
    let array = (in.sprite >> 16u) & 0xFu;
    let layer = in.sprite & 0xFFFFu;
    let texel = sample_sprite(array, in.uv, layer);
    var color = vec4<f32>(to_srgb(texel.rgb), texel.a) * in.color;
    if color.a < ALPHA_CUTOUT {
        discard;
    }
    if (in.sprite & GLINT_BIT) != 0u {
        let scaled = in.uv * sprite_px(array) / GLINT_ATLAS_PX * GLINT_SCALE;
        let rotated = vec2<f32>(
            scaled.x * cos(GLINT_ROTATION) - scaled.y * sin(GLINT_ROTATION),
            scaled.x * sin(GLINT_ROTATION) + scaled.y * cos(GLINT_ROTATION),
        );
        let glint_uv = rotated + vec2<f32>(-gui.glint_offset.x, gui.glint_offset.y);
        let sheen = to_srgb(textureSample(glint, glint_sampler, glint_uv).rgb) * gui.glint_alpha;
        color = vec4<f32>(color.rgb + sheen * sheen, max(color.a, gui.glint_alpha));
    }
    return vec4<f32>(to_linear(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), color.a);
}
