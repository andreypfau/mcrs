import package::shaders::include::fields::QUAD_WORDS;
import package::shaders::include::frame::params;
import package::shaders::include::quad::quad_of;
import package::shaders::include::section::SectionDesc;
import package::shaders::include::words::bits;
@if(MINIMAL) import package::shaders::include::words::word_texel;

struct AnimationFrame {
    layer: u32,
    next: u32,
    blend: f32,
};

struct SpriteEntry {
    array_layer: u32,
    animation: u32,
};

const STILL: u32 = 0xFFFFFFFFu;

// The arrays of words are storage buffers on the full path and integer textures on the minimal
// one, read through the functions below either way.
@if(MINIMAL) @group(1) @binding(0) var quads: texture_2d_array<u32>;
@else @group(1) @binding(0) var<storage, read> quads: array<u32>;
@if(MINIMAL) @group(1) @binding(1) var vertices: texture_2d_array<u32>;
@else @group(1) @binding(1) var<storage, read> vertices: array<u32>;
@if(!MINIMAL) @group(1) @binding(2) var<storage, read> visible: array<vec2<u32>>;
@group(1) @binding(3) var atlas0: texture_2d_array<f32>;
@group(1) @binding(4) var atlas1: texture_2d_array<f32>;
@group(1) @binding(5) var atlas2: texture_2d_array<f32>;
@group(1) @binding(6) var atlas3: texture_2d_array<f32>;
@group(1) @binding(7) var atlas_sampler: sampler;
@group(1) @binding(8) var tints: texture_2d_array<f32>;
@group(1) @binding(9) var tint_sampler: sampler;
@if(MINIMAL) @group(1) @binding(10) var animations: texture_2d_array<u32>;
@else @group(1) @binding(10) var<storage, read> animations: array<u32>;
@if(MINIMAL) @group(1) @binding(11) var sprites: texture_2d_array<u32>;
@else @group(1) @binding(11) var<storage, read> sprites: array<u32>;
@if(MINIMAL) @group(1) @binding(12) var faces: texture_2d_array<u32>;
@else @group(1) @binding(12) var<storage, read> faces: array<u32>;
@if(MINIMAL) @group(1) @binding(13) var sections: texture_2d_array<u32>;
@else @group(1) @binding(13) var<storage, read> sections: array<u32>;
@group(1) @binding(14) var lightmap_levels: texture_2d<f32>;
@group(1) @binding(16) var lightmap_sampler: sampler;

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

const SECTION_WORDS: u32 = 5u;

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
    return SpriteEntry(sprite_word(sprite * 2u), sprite_word(sprite * 2u + 1u));
}

const ANIMATION_WORDS: u32 = 4u;

fn animation_frame(animation: u32) -> AnimationFrame {
    let base = animation * ANIMATION_WORDS;
    return AnimationFrame(
        animation_word(base),
        animation_word(base + 1u),
        bitcast<f32>(animation_word(base + 2u)),
    );
}

/// What a vertex is drawn from: its index, and on the minimal path the group it is an instance
/// of, as the first quad, the quad count and the section of the group's record.
struct TerrainVertex {
    @builtin(vertex_index) index: u32,
    @if(MINIMAL) @location(0) group: vec3<u32>,
}

/// The quad a vertex belongs to and the section the quad lies in. A group holds fewer quads than
/// an instance has room for, and `drawn` is false for a vertex past the last of them.
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
