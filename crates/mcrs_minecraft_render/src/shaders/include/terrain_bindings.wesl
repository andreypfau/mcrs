import mcrs_minecraft_render::shaders::include::fields::QUAD_WORDS;
import mcrs_minecraft_render::shaders::include::frame::params;
import mcrs_minecraft_render::shaders::include::quad::quad_of;
import mcrs_minecraft_render::shaders::include::section_layout::{SECTION_WORDS, SectionDesc};
import mcrs_minecraft_render::shaders::include::sprites_layout::{
    ANIMATION_WORDS, AnimationFrame, SPRITE_WORDS, SpriteEntry,
};
import mcrs_minecraft_render::shaders::include::terrain_layout::{
    animations, faces, quads, sections, sprites, vertices, visible,
};
import mcrs_minecraft_render::shaders::include::words::bits;
@if(MINIMAL) import mcrs_minecraft_render::shaders::include::words::word_texel;

@if(MINIMAL)
fn quad_word(index: u32) -> u32 {
    let at = word_texel(index);
    return textureLoad(quads, at.xy, at.z, 0).x;
}
@else
fn quad_word(index: u32) -> u32 {
    return quads[index];
}

@if(MINIMAL)
fn model_word(index: u32) -> u32 {
    let at = word_texel(index);
    return textureLoad(vertices, at.xy, at.z, 0).x;
}
@else
fn model_word(index: u32) -> u32 {
    return vertices[index];
}

@if(MINIMAL)
fn face_word(index: u32) -> u32 {
    let at = word_texel(index);
    return textureLoad(faces, at.xy, at.z, 0).x;
}
@else
fn face_word(index: u32) -> u32 {
    return faces[index];
}

@if(MINIMAL)
fn section_word(index: u32) -> u32 {
    let at = word_texel(index);
    return textureLoad(sections, at.xy, at.z, 0).x;
}
@else
fn section_word(index: u32) -> u32 {
    return sections[index];
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

@if(MINIMAL)
fn animation_word(index: u32) -> u32 {
    let at = word_texel(index);
    return textureLoad(animations, at.xy, at.z, 0).x;
}
@else
fn animation_word(index: u32) -> u32 {
    return animations[index];
}

fn section_desc(slot: u32) -> SectionDesc {
    let base = slot * SECTION_WORDS;
    return SectionDesc(
        bitcast<i32>(section_word(base)),
        bitcast<i32>(section_word(base + 1u)),
        bitcast<i32>(section_word(base + 2u)),
        section_word(base + 3u),
        section_word(base + 4u),
    );
}

fn sprite_entry(sprite: u32) -> SpriteEntry {
    let base = sprite * SPRITE_WORDS;
    return SpriteEntry(sprite_word(base), sprite_word(base + 1u));
}

fn animation_frame(animation: u32) -> AnimationFrame {
    let base = animation * ANIMATION_WORDS;
    return AnimationFrame(
        animation_word(base),
        animation_word(base + 1u),
        bitcast<f32>(animation_word(base + 2u)),
    );
}

struct TerrainVertex {
    @builtin(vertex_index) index: u32,
    @if(MINIMAL) @location(0) group: vec3<u32>,
}

struct QuadRef {
    quad: u32,
    section: u32,
    drawn: bool,
}

@if(MINIMAL)
fn quad_ref(in: TerrainVertex) -> QuadRef {
    let nth = quad_of(in.index);
    return QuadRef(in.group.x + nth, in.group.z, nth < in.group.y);
}
@else
fn quad_ref(in: TerrainVertex) -> QuadRef {
    let entry = visible[params.visible_base + quad_of(in.index)];
    return QuadRef(entry.x, entry.y, true);
}

fn quad_words(base: u32) -> array<u32, QUAD_WORDS> {
    var words: array<u32, QUAD_WORDS>;
    for (var word = 0u; word < QUAD_WORDS; word++) {
        words[word] = quad_word(base + word);
    }
    return words;
}

fn quad_field(base: u32, word: u32, shift: u32, count: u32) -> u32 {
    return bits(quad_word(base + word), shift, count);
}

fn model_field(base: u32, word: u32, shift: u32, count: u32) -> u32 {
    return bits(model_word(base + word), shift, count);
}

fn face_field(base: u32, word: u32, shift: u32, count: u32) -> u32 {
    return bits(face_word(base + word), shift, count);
}
