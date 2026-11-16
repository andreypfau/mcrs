// Generated from `shader_layout.rs`. `cargo nextest run -p mcrs_minecraft_render`
// checks it; `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_render` rewrites it.

struct SectionDesc {
    x: i32,
    y: i32,
    z: i32,
    scale: u32,
    face_base: u32,
}

const SECTION_WORDS: u32 = 5u;

