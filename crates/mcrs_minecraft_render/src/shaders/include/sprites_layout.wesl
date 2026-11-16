// Generated from `shader_layout.rs`. `cargo nextest run -p mcrs_minecraft_render`
// checks it; `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_render` rewrites it.

struct SpriteEntry {
    array_layer: u32,
    animation: u32,
}

const SPRITE_WORDS: u32 = 2u;

struct AnimationFrame {
    layer: u32,
    next: u32,
    blend: f32,
}

const ANIMATION_WORDS: u32 = 4u;

const STILL: u32 = 0xFFFFFFFFu;
