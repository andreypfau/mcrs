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
