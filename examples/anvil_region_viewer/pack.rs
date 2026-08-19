//! The bit layout of the packed geometry, in one place.
//!
//! Three sides have to agree on it: the mesher writes the words, `terrain.wgsl` reads them to
//! build vertices, and `cull.wgsl` reads the section number out of a group. WGSL cannot import
//! Rust, so both shaders declare their own `NAME_SHIFT` and `NAME_BITS` constants under the names
//! used here, and the test at the bottom reads the shader sources back and fails if any of them
//! drifts. A field whose width disagrees between the two sides does not fail to compile — it
//! silently draws the wrong sprite or puts a quad in the wrong place.

/// One field of a packed word: where it starts and how wide it is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub shift: u32,
    pub bits: u32,
}

impl Field {
    const fn new(shift: u32, bits: u32) -> Self {
        Self { shift, bits }
    }

    /// The largest value the field holds. Also its mask once shifted down.
    pub const fn max(self) -> u64 {
        (1u64 << self.bits) - 1
    }

    pub const fn pack(self, value: u64) -> u64 {
        (value & self.max()) << self.shift
    }

    pub const fn get(self, word: u64) -> u64 {
        (word >> self.shift) & self.max()
    }
}

/// Block y is signed and the packed coordinate is not, so every packed y carries this offset.
pub const Y_BIAS: i32 = 64;

// Greedy quad, low word: the anchor corner and the face it points at.
pub const QUAD_X: Field = Field::new(0, 10);
pub const QUAD_Y: Field = Field::new(10, 9);
pub const QUAD_Z: Field = Field::new(19, 10);
pub const QUAD_FACE: Field = Field::new(29, 3);

// Greedy quad, high word. Width and height are stored one less than they are, so that the full
// sixteen blocks a merged quad can span still fit in four bits.
pub const QUAD_W: Field = Field::new(0, 4);
pub const QUAD_H: Field = Field::new(4, 4);
pub const QUAD_LAYER: Field = Field::new(8, 9);
pub const QUAD_AO: Field = Field::new(17, 8);
pub const QUAD_LIGHT: Field = Field::new(25, 4);
pub const QUAD_FLIP: Field = Field::new(29, 1);
pub const QUAD_TINT: Field = Field::new(30, 2);

// The section number a culling group carries, in units of sixteen blocks, plus the face group its
// quads all point at.
pub const SECTION_X: Field = Field::new(0, 5);
pub const SECTION_Y: Field = Field::new(5, 5);
pub const SECTION_Z: Field = Field::new(10, 5);
pub const GROUP_FACE: Field = Field::new(15, 3);

/// The section number on its own, which is what the sight-line bitset is indexed by.
pub const SECTION_INDEX: Field = Field::new(0, 15);

// Model vertex, three words per corner. Positions are fixed point, the rest is per-corner shading.
pub const MODEL_X: Field = Field::new(0, 16);
pub const MODEL_Y: Field = Field::new(16, 16);
pub const MODEL_Z: Field = Field::new(0, 16);
pub const MODEL_U: Field = Field::new(16, 10);
pub const MODEL_TINT: Field = Field::new(26, 2);
/// Sixteen bits are free here, but the greedy path is the narrower of the two and a sprite has to
/// be addressable from both, so this field is deliberately no wider than [`QUAD_LAYER`].
pub const MODEL_LAYER: Field = Field::new(0, 9);
pub const MODEL_V: Field = Field::new(16, 10);
pub const MODEL_LIGHT: Field = Field::new(26, 4);
pub const MODEL_SHADE: Field = Field::new(30, 2);

/// How far outside its own block a baked model quad may reach — a fence arm, a rail on a slope.
/// The fixed-point coordinate is offset by it so the overhang still lands on a non-negative
/// number, and the culling box of a model stream is grown by it so a section on the edge of the
/// frustum takes the quad poking into frame with it.
pub const MODEL_OVERHANG: f32 = 2.0;

/// Fixed-point steps per block in a model coordinate.
pub const MODEL_STEPS: f32 = 32.0;

/// How many sprites the packed layer field can address.
pub const MAX_SPRITES: usize = 1 << QUAD_LAYER.bits;

/// The section number a group carries, without the face group. Also the index of the section in
/// the sight-line bitset, which is why the two must be built the same way.
pub const fn pack_section(sx: u32, sy: u32, sz: u32) -> u32 {
    (SECTION_X.pack(sx as u64) | SECTION_Y.pack(sy as u64) | SECTION_Z.pack(sz as u64)) as u32
}

/// The inverse of [`pack_section`], for the walk that steps between neighbouring sections.
pub const fn section_coords(section: u32) -> [i32; 3] {
    [
        SECTION_X.get(section as u64) as i32,
        SECTION_Y.get(section as u64) as i32,
        SECTION_Z.get(section as u64) as i32,
    ]
}

#[cfg(test)]
const FIELDS: &[(&str, Field)] = &[
    ("QUAD_X", QUAD_X),
    ("QUAD_Y", QUAD_Y),
    ("QUAD_Z", QUAD_Z),
    ("QUAD_FACE", QUAD_FACE),
    ("QUAD_W", QUAD_W),
    ("QUAD_H", QUAD_H),
    ("QUAD_LAYER", QUAD_LAYER),
    ("QUAD_AO", QUAD_AO),
    ("QUAD_LIGHT", QUAD_LIGHT),
    ("QUAD_FLIP", QUAD_FLIP),
    ("QUAD_TINT", QUAD_TINT),
    ("SECTION_X", SECTION_X),
    ("SECTION_Y", SECTION_Y),
    ("SECTION_Z", SECTION_Z),
    ("GROUP_FACE", GROUP_FACE),
    ("SECTION_INDEX", SECTION_INDEX),
    ("MODEL_X", MODEL_X),
    ("MODEL_Y", MODEL_Y),
    ("MODEL_Z", MODEL_Z),
    ("MODEL_U", MODEL_U),
    ("MODEL_TINT", MODEL_TINT),
    ("MODEL_LAYER", MODEL_LAYER),
    ("MODEL_V", MODEL_V),
    ("MODEL_LIGHT", MODEL_LIGHT),
    ("MODEL_SHADE", MODEL_SHADE),
];

#[cfg(test)]
const SCALARS: &[(&str, f64)] = &[
    ("Y_BIAS", Y_BIAS as f64),
    ("MODEL_OVERHANG", MODEL_OVERHANG as f64),
    ("MODEL_STEPS", MODEL_STEPS as f64),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// `const NAME: type = value;`, with the WGSL integer suffix dropped.
    fn declarations(source: &str) -> Vec<(&str, f64)> {
        source
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("const ")?;
                let (name, rest) = rest.split_once(':')?;
                let value = rest.split_once('=')?.1.trim().trim_end_matches(';').trim();
                Some((name.trim(), value.trim_end_matches('u').parse().ok()?))
            })
            .collect()
    }

    #[test]
    fn the_shaders_unpack_the_fields_the_mesher_packs() {
        let shaders = [
            ("terrain.wgsl", include_str!("terrain.wgsl")),
            ("cull.wgsl", include_str!("cull.wgsl")),
        ];
        let mut checked = 0;
        for (file, source) in shaders {
            for (name, value) in declarations(source) {
                if let Some(&(_, expected)) = SCALARS.iter().find(|(known, _)| *known == name) {
                    assert_eq!(value, expected, "{file} disagrees on {name}");
                    checked += 1;
                    continue;
                }
                let Some((field, part)) = name.rsplit_once('_') else {
                    continue;
                };
                let Some(&(_, field)) = FIELDS.iter().find(|(known, _)| *known == field) else {
                    continue;
                };
                let expected = match part {
                    "SHIFT" => field.shift,
                    "BITS" => field.bits,
                    _ => continue,
                };
                assert_eq!(value, expected as f64, "{file} disagrees on {name}");
                checked += 1;
            }
        }
        assert!(
            checked >= FIELDS.len(),
            "the shaders only declared {checked} of the packing constants, so most of the layout \
             is going unchecked"
        );
    }

    #[test]
    fn a_field_round_trips_through_its_own_word() {
        let word = QUAD_W.pack(11) | QUAD_LAYER.pack(300) | QUAD_TINT.pack(2);
        assert_eq!(QUAD_W.get(word), 11);
        assert_eq!(QUAD_LAYER.get(word), 300);
        assert_eq!(QUAD_TINT.get(word), 2);
        assert_eq!(QUAD_AO.get(word), 0, "a neighbour must stay clear");
    }

    #[test]
    fn no_field_of_a_word_overlaps_another() {
        let words = [
            ("quad lo", &[QUAD_X, QUAD_Y, QUAD_Z, QUAD_FACE][..]),
            (
                "quad hi",
                &[
                    QUAD_W, QUAD_H, QUAD_LAYER, QUAD_AO, QUAD_LIGHT, QUAD_FLIP, QUAD_TINT,
                ][..],
            ),
            ("group section", &[SECTION_X, SECTION_Y, SECTION_Z, GROUP_FACE][..]),
            ("model w0", &[MODEL_X, MODEL_Y][..]),
            ("model w1", &[MODEL_Z, MODEL_U, MODEL_TINT][..]),
            ("model w2", &[MODEL_LAYER, MODEL_V, MODEL_LIGHT, MODEL_SHADE][..]),
        ];
        for (word, fields) in words {
            let mut taken = 0u64;
            for field in fields {
                let bits = field.max() << field.shift;
                assert_eq!(taken & bits, 0, "{word} packs two fields into the same bits");
                assert!(
                    field.shift + field.bits <= 32,
                    "{word} has a field running off the end of the word"
                );
                taken |= bits;
            }
        }
    }
}
