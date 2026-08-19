//! The bit layout of the packed geometry, in one place.
//!
//! Three sides have to agree on it: the mesher writes the words, `terrain.wgsl` reads them to
//! build vertices, and `cull.wgsl` reads the section number out of a group. WGSL cannot import
//! Rust, so both shaders declare their own `NAME_SHIFT` and `NAME_BITS` constants under the names
//! used here, and the test at the bottom reads the shader sources back and fails if any of them
//! drifts. A field whose width disagrees between the two sides does not fail to compile — it
//! silently draws the wrong sprite or puts a quad in the wrong place.

/// One field of a packed value: which word of it the field lives in, where it starts and how wide
/// it is. The word is part of the layout and not a choice the reader makes: moving a field from one
/// word to another while a read site keeps the old one produces a frame that is wrong without
/// anything failing to compile.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub word: u32,
    pub shift: u32,
    pub bits: u32,
}

impl Field {
    const fn new(word: u32, shift: u32, bits: u32) -> Self {
        Self { word, shift, bits }
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

    /// Writes the value into whichever word of `words` the field belongs to.
    pub fn set(self, words: &mut [u32], value: u64) {
        words[self.word as usize] |= self.pack(value) as u32;
    }

    #[cfg(test)]
    pub fn read(self, words: &[u32]) -> u64 {
        self.get(words[self.word as usize] as u64)
    }
}

// A render region is what one `draw_indirect` covers. A quad carries the number of its section
// inside the region, and the region's own corner arrives in that draw's uniform, so a coordinate
// never has to be wide enough to name a place in the world — only a place in a section.
//
// The shape below is a size trade, not a natural constant. A larger region spends more bits on the
// section number and leaves fewer for the sprite; a smaller one spends fewer bits but costs more
// draws, and this backend has no multi-draw indirect to fold them back into one. Sixteen by eight
// by sixteen sections holds the section number to eleven bits while keeping a wide view in the low
// thousands of draws.
pub const RENDER_REGION_X: usize = 16;
pub const RENDER_REGION_Y: usize = 8;
pub const RENDER_REGION_Z: usize = 16;
pub const SECTIONS_PER_RENDER_REGION: usize =
    RENDER_REGION_X * RENDER_REGION_Y * RENDER_REGION_Z;

// The section number, as carried by a quad and by the group that culls it.
pub const LOCAL_X: Field = Field::new(0, 0, 4);
pub const LOCAL_Y: Field = Field::new(0, 4, 3);
pub const LOCAL_Z: Field = Field::new(0, 7, 4);

/// The section number as a whole, which is also its offset into the region's slice of the
/// sight-line bitset.
pub const SECTION_INDEX: Field = Field::new(0, 0, 11);

/// The face group a culling group's quads all point at, above the section number it carries.
pub const GROUP_FACE: Field = Field::new(0, SECTION_INDEX.bits, 3);

// Greedy quad, low word. The anchor is relative to its own section and runs 0..16 inclusive,
// because a quad on the far face of a section anchors on the boundary. Width and height are stored
// one less than they are, so the full sixteen blocks a merged quad can span still fit in four bits.
pub const QUAD_X: Field = Field::new(0, 0, 5);
pub const QUAD_Y: Field = Field::new(0, 5, 5);
pub const QUAD_Z: Field = Field::new(0, 10, 5);
pub const QUAD_FACE: Field = Field::new(0, 15, 3);
pub const QUAD_W: Field = Field::new(0, 18, 4);
pub const QUAD_H: Field = Field::new(0, 22, 4);
pub const QUAD_LIGHT: Field = Field::new(0, 26, 4);
pub const QUAD_TINT: Field = Field::new(0, 30, 2);

// Greedy quad, high word. The layer sits at the top so it can widen into the bit above it without
// moving anything else.
pub const QUAD_AO: Field = Field::new(1, 0, 8);
pub const QUAD_FLIP: Field = Field::new(1, 8, 1);
pub const QUAD_SECTION: Field = Field::new(1, 9, SECTION_INDEX.bits);
/// Which sprite array the layer indexes. Arrays are split by sprite size, so the pack decides how
/// many there are rather than the format.
pub const QUAD_ARRAY: Field = Field::new(1, 20, 2);
pub const QUAD_LAYER: Field = Field::new(1, 22, 10);

// Model vertex, three words per corner. Positions are fixed point relative to the section, wide
// enough for the overhang on both sides.
pub const MODEL_X: Field = Field::new(0, 0, 10);
pub const MODEL_Y: Field = Field::new(0, 10, 10);
pub const MODEL_Z: Field = Field::new(0, 20, 10);
pub const MODEL_U: Field = Field::new(1, 0, 10);
pub const MODEL_V: Field = Field::new(1, 10, 10);
pub const MODEL_TINT: Field = Field::new(1, 20, 2);
pub const MODEL_LIGHT: Field = Field::new(1, 22, 4);
pub const MODEL_SHADE: Field = Field::new(1, 26, 2);
pub const MODEL_SECTION: Field = Field::new(2, 0, SECTION_INDEX.bits);
pub const MODEL_ARRAY: Field = Field::new(2, SECTION_INDEX.bits, QUAD_ARRAY.bits);
/// A sprite has to be addressable from the greedy path too, so this is no wider than
/// [`QUAD_LAYER`] however much room the model vertex has left.
pub const MODEL_LAYER: Field =
    Field::new(2, SECTION_INDEX.bits + QUAD_ARRAY.bits, QUAD_LAYER.bits);

/// How far outside its own block a baked model quad may reach — a fence arm, a rail on a slope.
/// The fixed-point coordinate is offset by it so the overhang still lands on a non-negative
/// number, and the culling box of a model stream is grown by it so a section on the edge of the
/// frustum takes the quad poking into frame with it.
pub const MODEL_OVERHANG: f32 = 2.0;

/// Fixed-point steps per block in a model coordinate.
pub const MODEL_STEPS: f32 = 32.0;

/// How many sprites one array can hold. With four arrays that is four thousand addressable
/// sprites, against the eleven hundred a vanilla pack defines and the seventeen hundred it would
/// reach with every animation frame unrolled.
pub const MAX_SPRITES: usize = 1 << QUAD_LAYER.bits;

/// How many sprite arrays the format can address, and so how many resolutions a pack may use.
pub const MAX_SPRITE_ARRAYS: usize = 1 << QUAD_ARRAY.bits;

/// The grid of render regions a loaded world is cut into. Every section belongs to exactly one,
/// and a section's number is only meaningful next to the region that contains it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RegionGrid {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl RegionGrid {
    pub fn covering(sections: [usize; 3]) -> Self {
        Self {
            x: sections[0].div_ceil(RENDER_REGION_X),
            y: sections[1].div_ceil(RENDER_REGION_Y),
            z: sections[2].div_ceil(RENDER_REGION_Z),
        }
    }

    pub const fn len(self) -> usize {
        self.x * self.y * self.z
    }

    /// Which region a section belongs to, and where it sits inside that region.
    pub const fn split(self, sx: usize, sy: usize, sz: usize) -> (usize, u32) {
        let region = sx / RENDER_REGION_X
            + sz / RENDER_REGION_Z * self.x
            + sy / RENDER_REGION_Y * self.x * self.z;
        let local = pack_section(
            (sx % RENDER_REGION_X) as u32,
            (sy % RENDER_REGION_Y) as u32,
            (sz % RENDER_REGION_Z) as u32,
        );
        (region, local)
    }

    /// A section's index in the one space the sight-line walk, the bitset the walk fills and the
    /// culling shader all share. Keeping them on one space is what stops the culling from drifting
    /// out of step with the geometry.
    pub const fn slot(self, sx: usize, sy: usize, sz: usize) -> usize {
        let (region, local) = self.split(sx, sy, sz);
        region * SECTIONS_PER_RENDER_REGION + local as usize
    }

    /// The inverse of [`Self::slot`].
    pub const fn section_at(self, slot: usize) -> [usize; 3] {
        let [cx, sy, cz] = self.corner(slot / SECTIONS_PER_RENDER_REGION);
        let [lx, ly, lz] = section_coords((slot % SECTIONS_PER_RENDER_REGION) as u32);
        [cx + lx as usize, sy + ly as usize, cz + lz as usize]
    }

    /// How many section slots the grid spans, padding included.
    pub const fn slots(self) -> usize {
        self.len() * SECTIONS_PER_RENDER_REGION
    }

    /// The section coordinates of a region's own corner.
    pub const fn corner(self, region: usize) -> [usize; 3] {
        [
            region % self.x * RENDER_REGION_X,
            region / (self.x * self.z) * RENDER_REGION_Y,
            region / self.x % self.z * RENDER_REGION_Z,
        ]
    }
}

/// Where a section sits inside its render region, which is what a quad and a group both carry.
pub const fn pack_section(lx: u32, ly: u32, lz: u32) -> u32 {
    (LOCAL_X.pack(lx as u64) | LOCAL_Y.pack(ly as u64) | LOCAL_Z.pack(lz as u64)) as u32
}

/// The inverse of [`pack_section`].
pub const fn section_coords(section: u32) -> [i32; 3] {
    [
        LOCAL_X.get(section as u64) as i32,
        LOCAL_Y.get(section as u64) as i32,
        LOCAL_Z.get(section as u64) as i32,
    ]
}

#[cfg(test)]
const FIELDS: &[(&str, Field)] = &[
    ("LOCAL_X", LOCAL_X),
    ("LOCAL_Y", LOCAL_Y),
    ("LOCAL_Z", LOCAL_Z),
    ("SECTION_INDEX", SECTION_INDEX),
    ("GROUP_FACE", GROUP_FACE),
    ("QUAD_X", QUAD_X),
    ("QUAD_Y", QUAD_Y),
    ("QUAD_Z", QUAD_Z),
    ("QUAD_FACE", QUAD_FACE),
    ("QUAD_W", QUAD_W),
    ("QUAD_H", QUAD_H),
    ("QUAD_LIGHT", QUAD_LIGHT),
    ("QUAD_TINT", QUAD_TINT),
    ("QUAD_AO", QUAD_AO),
    ("QUAD_FLIP", QUAD_FLIP),
    ("QUAD_SECTION", QUAD_SECTION),
    ("QUAD_ARRAY", QUAD_ARRAY),
    ("QUAD_LAYER", QUAD_LAYER),
    ("MODEL_X", MODEL_X),
    ("MODEL_Y", MODEL_Y),
    ("MODEL_Z", MODEL_Z),
    ("MODEL_U", MODEL_U),
    ("MODEL_V", MODEL_V),
    ("MODEL_TINT", MODEL_TINT),
    ("MODEL_LIGHT", MODEL_LIGHT),
    ("MODEL_SHADE", MODEL_SHADE),
    ("MODEL_SECTION", MODEL_SECTION),
    ("MODEL_ARRAY", MODEL_ARRAY),
    ("MODEL_LAYER", MODEL_LAYER),
];

#[cfg(test)]
const SCALARS: &[(&str, f64)] = &[
    ("SECTION_SIZE", crate::anvil::SECTION_SIZE as f64),
    ("MODEL_OVERHANG", MODEL_OVERHANG as f64),
    ("MODEL_STEPS", MODEL_STEPS as f64),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anvil::{REGION_CHUNKS, SECTION_SIZE};

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
                    "WORD" => field.word,
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
        let word = QUAD_W.pack(11) | QUAD_TINT.pack(2) | QUAD_FACE.pack(5);
        assert_eq!(QUAD_W.get(word), 11);
        assert_eq!(QUAD_TINT.get(word), 2);
        assert_eq!(QUAD_FACE.get(word), 5);
        assert_eq!(QUAD_H.get(word), 0, "a neighbour must stay clear");
    }

    /// Both words of a quad are full, so a field that grew without another shrinking would run off
    /// the end rather than quietly overlap a neighbour.
    #[test]
    fn a_quad_still_fits_in_its_eight_bytes() {
        let quad = [
            QUAD_X, QUAD_Y, QUAD_Z, QUAD_FACE, QUAD_W, QUAD_H, QUAD_LIGHT, QUAD_TINT, QUAD_AO,
            QUAD_FLIP, QUAD_SECTION, QUAD_ARRAY, QUAD_LAYER,
        ];
        let bits: u32 = quad.iter().map(|field| field.bits).sum();
        assert_eq!(bits, 64);
    }

    #[test]
    fn a_section_number_round_trips_and_spans_the_whole_region() {
        let corner = pack_section(
            RENDER_REGION_X as u32 - 1,
            RENDER_REGION_Y as u32 - 1,
            RENDER_REGION_Z as u32 - 1,
        );
        assert_eq!(
            section_coords(corner),
            [
                RENDER_REGION_X as i32 - 1,
                RENDER_REGION_Y as i32 - 1,
                RENDER_REGION_Z as i32 - 1
            ]
        );
        assert_eq!(
            corner as u64,
            SECTION_INDEX.max(),
            "the far corner has to be the last slot, or the bitset has holes in it"
        );
        assert_eq!(SECTIONS_PER_RENDER_REGION, 1 << SECTION_INDEX.bits);
    }

    #[test]
    fn every_section_of_a_grid_gets_its_own_slot() {
        let sections = [REGION_CHUNKS, 24, REGION_CHUNKS];
        let grid = RegionGrid::covering(sections);
        let mut seen = vec![false; grid.slots()];
        for sz in 0..sections[2] {
            for sy in 0..sections[1] {
                for sx in 0..sections[0] {
                    let slot = grid.slot(sx, sy, sz);
                    assert!(!seen[slot], "two sections share slot {slot}");
                    seen[slot] = true;
                    assert_eq!(grid.section_at(slot), [sx, sy, sz]);
                }
            }
        }
    }

    #[test]
    fn a_model_coordinate_reaches_the_overhang_on_both_sides() {
        let far = (SECTION_SIZE as f32 + MODEL_OVERHANG + MODEL_OVERHANG) * MODEL_STEPS;
        assert!(
            far <= MODEL_X.max() as f32,
            "a model quad hanging {MODEL_OVERHANG} blocks past a section does not fit the field"
        );
    }

    #[test]
    fn no_field_of_a_word_overlaps_another() {
        let values = [
            (
                "greedy quad",
                &[
                    QUAD_X, QUAD_Y, QUAD_Z, QUAD_FACE, QUAD_W, QUAD_H, QUAD_LIGHT, QUAD_TINT,
                    QUAD_AO, QUAD_FLIP, QUAD_SECTION, QUAD_ARRAY, QUAD_LAYER,
                ][..],
            ),
            ("section number", &[LOCAL_X, LOCAL_Y, LOCAL_Z][..]),
            ("group section", &[SECTION_INDEX, GROUP_FACE][..]),
            (
                "model vertex",
                &[
                    MODEL_X, MODEL_Y, MODEL_Z, MODEL_U, MODEL_V, MODEL_TINT, MODEL_LIGHT,
                    MODEL_SHADE, MODEL_SECTION, MODEL_ARRAY, MODEL_LAYER,
                ][..],
            ),
        ];
        for (value, fields) in values {
            let mut taken = [0u64; 4];
            for field in fields {
                let bits = field.max() << field.shift;
                let word = &mut taken[field.word as usize];
                assert_eq!(
                    *word & bits,
                    0,
                    "{value} packs two fields into the same bits of word {}",
                    field.word
                );
                assert!(
                    field.shift + field.bits <= 32,
                    "{value} has a field running off the end of word {}",
                    field.word
                );
                *word |= bits;
            }
        }
    }
}
