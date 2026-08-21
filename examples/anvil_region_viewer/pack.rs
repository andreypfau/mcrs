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

    pub const fn max(self) -> u64 {
        (1u64 << self.bits) - 1
    }

    pub const fn pack(self, value: u64) -> u64 {
        assert!(value <= self.max(), "a value does not fit the field it is packed into");
        (value & self.max()) << self.shift
    }

    pub const fn get(self, word: u64) -> u64 {
        (word >> self.shift) & self.max()
    }

    pub fn set(self, words: &mut [u32], value: u64) {
        words[self.word as usize] |= self.pack(value) as u32;
    }

    #[cfg(test)]
    pub fn read(self, words: &[u32]) -> u64 {
        self.get(words[self.word as usize] as u64)
    }
}

pub const RENDER_REGION_X: usize = 16;
pub const RENDER_REGION_Y: usize = 8;
pub const RENDER_REGION_Z: usize = 16;
pub const SECTIONS_PER_RENDER_REGION: usize =
    RENDER_REGION_X * RENDER_REGION_Y * RENDER_REGION_Z;

pub const LOCAL_X: Field = Field::new(0, 0, 4);
pub const LOCAL_Y: Field = Field::new(0, 4, 3);
pub const LOCAL_Z: Field = Field::new(0, 7, 4);

pub const SECTION_INDEX: Field = Field::new(0, 0, 11);

pub const GROUP_FACE: Field = Field::new(0, SECTION_INDEX.bits, 4);

pub const QUAD_X: Field = Field::new(0, 0, 5);
pub const QUAD_Y: Field = Field::new(0, 5, 5);
pub const QUAD_Z: Field = Field::new(0, 10, 5);
pub const QUAD_FACE: Field = Field::new(0, 15, 3);
pub const QUAD_W: Field = Field::new(0, 18, 4);
pub const QUAD_H: Field = Field::new(0, 22, 4);
pub const QUAD_DROP: Field = Field::new(0, 26, 5);
pub const QUAD_FLUID: Field = Field::new(0, 31, 1);

pub const QUAD_SECTION: Field = Field::new(1, 0, SECTION_INDEX.bits);
pub const QUAD_FACE_BASE: Field = Field::new(1, SECTION_INDEX.bits, 16);

pub const QUAD_WORDS: usize = 2;

pub const SECTION_FACE_TABLE: usize = SECTIONS_PER_RENDER_REGION;

const _: () = assert!(
    (12 * crate::anvil::SECTION_VOLUME) as u64 <= QUAD_FACE_BASE.max(),
    "a section can hold more faces than a quad can name a place among"
);

pub const FACE_LAYER: Field = Field::new(0, 0, 10);
pub const FACE_ARRAY: Field = Field::new(0, 10, 2);
pub const FACE_TINT: Field = Field::new(0, 12, 2);
pub const FACE_BLOCK_LIGHT: Field = Field::new(0, 14, 4);
pub const FACE_SKY_LIGHT: Field = Field::new(0, 18, 4);
pub const FACE_AO: Field = Field::new(0, 22, 8);
pub const FACE_FLUID: Field = Field::new(0, 30, 1);

pub const MODEL_X: Field = Field::new(0, 0, 10);
pub const MODEL_Y: Field = Field::new(0, 10, 10);
pub const MODEL_Z: Field = Field::new(0, 20, 10);
pub const MODEL_U: Field = Field::new(1, 0, 10);
pub const MODEL_V: Field = Field::new(1, 10, 10);
pub const MODEL_TINT: Field = Field::new(1, 20, 2);
pub const MODEL_BLOCK_LIGHT: Field = Field::new(1, 22, 4);
pub const MODEL_SHADE: Field = Field::new(1, 26, 2);
pub const MODEL_SKY_LIGHT: Field = Field::new(1, 28, 4);
pub const MODEL_SECTION: Field = Field::new(2, 0, SECTION_INDEX.bits);
pub const MODEL_ARRAY: Field = Field::new(2, SECTION_INDEX.bits, FACE_ARRAY.bits);
pub const MODEL_LAYER: Field =
    Field::new(2, SECTION_INDEX.bits + FACE_ARRAY.bits, FACE_LAYER.bits);

pub const MODEL_OVERHANG: f32 = 2.0;

pub const MODEL_STEPS: f32 = 32.0;

pub const FLUID_INSET: f32 = 0.001;

pub const FACE_NONE: u32 = 10;

pub const MAX_SPRITES: usize = 1 << FACE_LAYER.bits;

pub const MAX_SPRITE_ARRAYS: usize = 1 << FACE_ARRAY.bits;

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

    pub const fn slot(self, sx: usize, sy: usize, sz: usize) -> usize {
        let (region, local) = self.split(sx, sy, sz);
        region * SECTIONS_PER_RENDER_REGION + local as usize
    }

    pub const fn section_at(self, slot: usize) -> [usize; 3] {
        let [cx, sy, cz] = self.corner(slot / SECTIONS_PER_RENDER_REGION);
        let [lx, ly, lz] = section_coords((slot % SECTIONS_PER_RENDER_REGION) as u32);
        [cx + lx as usize, sy + ly as usize, cz + lz as usize]
    }

    pub const fn extent(self) -> [usize; 3] {
        [
            self.x * RENDER_REGION_X,
            self.y * RENDER_REGION_Y,
            self.z * RENDER_REGION_Z,
        ]
    }

    pub const fn slots(self) -> usize {
        self.len() * SECTIONS_PER_RENDER_REGION
    }

    pub const fn origin(self, min_section: [i32; 3], region: usize) -> [i32; 3] {
        let [sx, sy, sz] = self.corner(region);
        let size = crate::anvil::SECTION_SIZE as i32;
        [
            (sx as i32 + min_section[0]) * size,
            (sy as i32 + min_section[1]) * size,
            (sz as i32 + min_section[2]) * size,
        ]
    }

    pub const fn corner(self, region: usize) -> [usize; 3] {
        [
            region % self.x * RENDER_REGION_X,
            region / (self.x * self.z) * RENDER_REGION_Y,
            region / self.x % self.z * RENDER_REGION_Z,
        ]
    }
}

pub const fn pack_section(lx: u32, ly: u32, lz: u32) -> u32 {
    (LOCAL_X.pack(lx as u64) | LOCAL_Y.pack(ly as u64) | LOCAL_Z.pack(lz as u64)) as u32
}

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
    ("QUAD_DROP", QUAD_DROP),
    ("QUAD_FLUID", QUAD_FLUID),
    ("QUAD_SECTION", QUAD_SECTION),
    ("QUAD_FACE_BASE", QUAD_FACE_BASE),
    ("FACE_LAYER", FACE_LAYER),
    ("FACE_ARRAY", FACE_ARRAY),
    ("FACE_TINT", FACE_TINT),
    ("FACE_BLOCK_LIGHT", FACE_BLOCK_LIGHT),
    ("FACE_SKY_LIGHT", FACE_SKY_LIGHT),
    ("FACE_AO", FACE_AO),
    ("FACE_FLUID", FACE_FLUID),
    ("MODEL_X", MODEL_X),
    ("MODEL_Y", MODEL_Y),
    ("MODEL_Z", MODEL_Z),
    ("MODEL_U", MODEL_U),
    ("MODEL_V", MODEL_V),
    ("MODEL_TINT", MODEL_TINT),
    ("MODEL_BLOCK_LIGHT", MODEL_BLOCK_LIGHT),
    ("MODEL_SKY_LIGHT", MODEL_SKY_LIGHT),
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
    ("FLUID_INSET", FLUID_INSET as f64),
    ("QUAD_WORDS", QUAD_WORDS as f64),
    ("FACE_NONE", FACE_NONE as f64),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anvil::{REGION_CHUNKS, SECTION_SIZE};

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
            ("layout.wgsl", include_str!("render/shaders/layout.wgsl")),
            ("terrain.wgsl", include_str!("render/shaders/terrain.wgsl")),
            ("cull.wgsl", include_str!("render/shaders/cull.wgsl")),
        ];
        let mut seen: Vec<(&str, &str)> = Vec::new();
        for (file, source) in shaders {
            for (name, value) in declarations(source) {
                if let Some(&(_, expected)) = SCALARS.iter().find(|(known, _)| *known == name) {
                    assert_eq!(value as f32, expected as f32, "{file} disagrees on {name}");
                    seen.push((name, "SCALAR"));
                    continue;
                }
                let Some((field, part)) = name.rsplit_once('_') else {
                    continue;
                };
                let Some(&(known, field)) = FIELDS.iter().find(|(known, _)| *known == field) else {
                    continue;
                };
                let expected = match part {
                    "WORD" => field.word,
                    "SHIFT" => field.shift,
                    "BITS" => field.bits,
                    _ => continue,
                };
                assert_eq!(value, expected as f64, "{file} disagrees on {name}");
                seen.push((known, part));
            }
        }
        for (name, _) in FIELDS {
            for part in ["WORD", "SHIFT", "BITS"] {
                assert!(
                    seen.contains(&(name, part)),
                    "no shader declares {name}_{part}, so that much of the layout is unchecked"
                );
            }
        }
        for (name, _) in SCALARS {
            assert!(
                seen.contains(&(*name, "SCALAR")),
                "no shader declares {name}, so it is unchecked"
            );
        }
    }

    #[test]
    fn a_field_round_trips_through_its_own_word() {
        let word = QUAD_W.pack(11) | QUAD_X.pack(2) | QUAD_FACE.pack(5);
        assert_eq!(QUAD_W.get(word), 11);
        assert_eq!(QUAD_X.get(word), 2);
        assert_eq!(QUAD_FACE.get(word), 5);
        assert_eq!(QUAD_H.get(word), 0, "a neighbour must stay clear");
    }

    #[test]
    fn the_three_model_axes_are_the_same_width() {
        assert_eq!(MODEL_X.bits, MODEL_Y.bits);
        assert_eq!(MODEL_X.bits, MODEL_Z.bits);
        assert_eq!(MODEL_U.bits, MODEL_V.bits);
    }

    #[test]
    #[should_panic(expected = "does not fit")]
    fn packing_a_value_too_wide_for_its_field_is_caught() {
        QUAD_FACE.pack(8);
    }

    #[test]
    fn no_word_of_a_quad_is_overfull() {
        let quad = [
            QUAD_X, QUAD_Y, QUAD_Z, QUAD_FACE, QUAD_W, QUAD_H, QUAD_DROP, QUAD_FLUID,
            QUAD_SECTION, QUAD_FACE_BASE,
        ];
        for word in 0..QUAD_WORDS as u32 {
            let bits: u32 = quad
                .iter()
                .filter(|field| field.word == word)
                .map(|field| field.bits)
                .sum();
            assert!(bits <= 32, "word {word} of a quad holds {bits} bits");
        }
        assert!(quad.iter().all(|field| (field.word as usize) < QUAD_WORDS));
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
    fn a_region_below_the_origin_keeps_the_whole_corner_of_its_window() {
        let grid = RegionGrid::covering([RENDER_REGION_X * 2, RENDER_REGION_Y, RENDER_REGION_Z * 2]);
        assert_eq!(grid.origin([-32, -4, -32], 0), [-512, -64, -512]);
        assert_eq!(
            grid.origin([-32, -4, -32], 3),
            [-512 + RENDER_REGION_X as i32 * 16, -64, -512 + RENDER_REGION_Z as i32 * 16],
            "the far corner of a two-by-two grid"
        );
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
                &[QUAD_X, QUAD_Y, QUAD_Z, QUAD_FACE, QUAD_W, QUAD_H, QUAD_SECTION, QUAD_FACE_BASE]
                    [..],
            ),
            (
                "face attribute",
                &[
                    FACE_LAYER, FACE_ARRAY, FACE_TINT, FACE_BLOCK_LIGHT, FACE_SKY_LIGHT, FACE_AO,
                ][..],
            ),
            ("section number", &[LOCAL_X, LOCAL_Y, LOCAL_Z][..]),
            ("group section", &[SECTION_INDEX, GROUP_FACE][..]),
            (
                "model vertex",
                &[
                    MODEL_X, MODEL_Y, MODEL_Z, MODEL_U, MODEL_V, MODEL_TINT, MODEL_BLOCK_LIGHT,
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
