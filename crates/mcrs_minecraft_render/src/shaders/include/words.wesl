/// `extractBits` written out: GLSL ES 3.0, which WebGL2 speaks, has no bit field functions for it
/// to be translated into. No field is a whole word wide.
fn bits(word: u32, shift: u32, count: u32) -> u32 {
    return (word >> shift) & ((1u << count) - 1u);
}

@if(MINIMAL)
const WORD_ROW_BITS: u32 = constants::WORD_ROW_BITS;

/// Where word `index` of a word texture sits: `xy` in its layer and `z` the layer. A layer holds
/// half as many rows as a row holds words.
@if(MINIMAL)
fn word_texel(index: u32) -> vec3<u32> {
    let row = index >> WORD_ROW_BITS;
    let layer_bits = WORD_ROW_BITS - 1u;
    return vec3<u32>(index & ((1u << WORD_ROW_BITS) - 1u), row & ((1u << layer_bits) - 1u), row >> layer_bits);
}
