import mcrs_minecraft_render::shaders::include::sprites_layout::{
    ANIMATION_WORDS, AnimationFrame, SPRITE_WORDS, STILL, SpriteEntry,
};
@if(MINIMAL) import mcrs_minecraft_render::shaders::include::words::word_texel;

struct GuiUniform {
    framebuffer: vec2<f32>,
    glint_offset: vec2<f32>,
    scale: f32,
    glint_alpha: f32,
}

@group(0) @binding(0) var<uniform> gui: GuiUniform;
@group(0) @binding(1) var atlas0: texture_2d_array<f32>;
@group(0) @binding(2) var atlas1: texture_2d_array<f32>;
@group(0) @binding(3) var atlas2: texture_2d_array<f32>;
@group(0) @binding(4) var atlas3: texture_2d_array<f32>;
@group(0) @binding(5) var atlas_sampler: sampler;
@if(MINIMAL) @group(0) @binding(6) var animations: texture_2d_array<u32>;
@else @group(0) @binding(6) var<storage, read> animations: array<u32>;
@if(MINIMAL) @group(0) @binding(7) var sprites: texture_2d_array<u32>;
@else @group(0) @binding(7) var<storage, read> sprites: array<u32>;
@group(0) @binding(8) var gui_atlas: texture_2d<f32>;
@group(0) @binding(9) var glint: texture_2d<f32>;
@group(0) @binding(10) var glint_sampler: sampler;

@if(MINIMAL)
fn animation_word(index: u32) -> u32 {
    let at = word_texel(index);
    return textureLoad(animations, at.xy, at.z, 0).x;
}
@else
fn animation_word(index: u32) -> u32 {
    return animations[index];
}

@if(MINIMAL)
fn sprite_word(index: u32) -> u32 {
    let at = word_texel(index);
    return textureLoad(sprites, at.xy, at.z, 0).x;
}
@else
fn sprite_word(index: u32) -> u32 {
    return sprites[index];
}

fn animation_frame(animation: u32) -> AnimationFrame {
    let base = animation * ANIMATION_WORDS;
    return AnimationFrame(
        animation_word(base),
        animation_word(base + 1u),
        bitcast<f32>(animation_word(base + 2u)),
    );
}

fn sprite_entry(sprite: u32) -> SpriteEntry {
    let base = sprite * SPRITE_WORDS;
    return SpriteEntry(sprite_word(base), sprite_word(base + 1u));
}

const GUI_ATLAS_BIT: u32 = 0x80000000u;
const GLINT_BIT: u32 = 0x40000000u;
const ALPHA_CUTOUT: f32 = 0.1;
const GLINT_ROTATION: f32 = 0.17453292;
const GLINT_SCALE: f32 = 8.0;
const GLINT_SPRITE_PX: f32 = 16.0;
const ITEM_ATLAS_BIT: u32 = 0x20000000u;
const BLOCK_ATLAS_PX: vec2<f32> = vec2<f32>(2048.0, 2048.0);
const ITEM_ATLAS_PX: vec2<f32> = vec2<f32>(1024.0, 512.0);

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

fn sample_sprite(entry: SpriteEntry, uv: vec2<f32>) -> vec4<f32> {
    let array = entry.array_layer >> 16u;
    if entry.animation == STILL {
        return sample_atlas(array, uv, entry.array_layer & 0xFFFFu);
    }
    let frame = animation_frame(entry.animation);
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

@fragment
fn fs_gui(in: VertexOut) -> @location(0) vec4<f32> {
    if (in.sprite & GUI_ATLAS_BIT) != 0u {
        let texel = textureSampleLevel(gui_atlas, atlas_sampler, in.uv, 0.0);
        let color = vec4<f32>(to_srgb(texel.rgb), texel.a) * in.color;
        return vec4<f32>(to_linear(color.rgb), color.a);
    }
    let entry = sprite_entry(in.sprite & 0xFFFFu);
    var color = sample_sprite(entry, in.uv) * in.color;
    if color.a < ALPHA_CUTOUT {
        discard;
    }
    if (in.sprite & GLINT_BIT) != 0u {
        let atlas_px = select(BLOCK_ATLAS_PX, ITEM_ATLAS_PX, (in.sprite & ITEM_ATLAS_BIT) != 0u);
        let scaled = in.uv * GLINT_SPRITE_PX / atlas_px * GLINT_SCALE;
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
