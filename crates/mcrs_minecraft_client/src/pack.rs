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
        assert!(
            value <= self.max(),
            "a value does not fit the field it is packed into"
        );
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

pub const QUAD_X: Field = Field::new(0, 0, 5);
pub const QUAD_Y: Field = Field::new(0, 5, 5);
pub const QUAD_Z: Field = Field::new(0, 10, 5);
pub const QUAD_FACE: Field = Field::new(0, 15, 3);
pub const QUAD_W: Field = Field::new(0, 18, 4);
pub const QUAD_H: Field = Field::new(0, 22, 4);
pub const QUAD_DROP: Field = Field::new(0, 26, 5);
pub const QUAD_FLUID: Field = Field::new(0, 31, 1);

pub const QUAD_FACE_BASE: Field = Field::new(1, 0, 16);

pub const QUAD_WORDS: usize = 2;

const _: () = assert!(
    (12 * mcrs_minecraft_network::columns::SECTION_VOLUME) as u64 <= QUAD_FACE_BASE.max(),
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
pub const MODEL_ARRAY: Field = Field::new(2, 0, FACE_ARRAY.bits);
pub const MODEL_LAYER: Field = Field::new(2, FACE_ARRAY.bits, FACE_LAYER.bits);

pub const MODEL_OVERHANG: f32 = 2.0;

pub const MODEL_STEPS: f32 = 32.0;

pub const FLUID_INSET: f32 = 0.001;

pub const FACE_NONE: u32 = 10;

pub const MAX_SPRITES: usize = 1 << FACE_LAYER.bits;

pub const MAX_SPRITE_ARRAYS: usize = 1 << FACE_ARRAY.bits;

#[cfg(test)]
const QUAD_FIELDS: &[(&str, Field)] = &[
    ("QUAD_X", QUAD_X),
    ("QUAD_Y", QUAD_Y),
    ("QUAD_Z", QUAD_Z),
    ("QUAD_FACE", QUAD_FACE),
    ("QUAD_W", QUAD_W),
    ("QUAD_H", QUAD_H),
    ("QUAD_DROP", QUAD_DROP),
    ("QUAD_FLUID", QUAD_FLUID),
    ("QUAD_FACE_BASE", QUAD_FACE_BASE),
];

#[cfg(test)]
const FACE_FIELDS: &[(&str, Field)] = &[
    ("FACE_LAYER", FACE_LAYER),
    ("FACE_ARRAY", FACE_ARRAY),
    ("FACE_TINT", FACE_TINT),
    ("FACE_BLOCK_LIGHT", FACE_BLOCK_LIGHT),
    ("FACE_SKY_LIGHT", FACE_SKY_LIGHT),
    ("FACE_AO", FACE_AO),
    ("FACE_FLUID", FACE_FLUID),
];

#[cfg(test)]
const MODEL_FIELDS: &[(&str, Field)] = &[
    ("MODEL_X", MODEL_X),
    ("MODEL_Y", MODEL_Y),
    ("MODEL_Z", MODEL_Z),
    ("MODEL_U", MODEL_U),
    ("MODEL_V", MODEL_V),
    ("MODEL_TINT", MODEL_TINT),
    ("MODEL_BLOCK_LIGHT", MODEL_BLOCK_LIGHT),
    ("MODEL_SKY_LIGHT", MODEL_SKY_LIGHT),
    ("MODEL_SHADE", MODEL_SHADE),
    ("MODEL_ARRAY", MODEL_ARRAY),
    ("MODEL_LAYER", MODEL_LAYER),
];

#[cfg(test)]
const GROUPS: &[(&str, &[(&str, Field)])] = &[
    ("greedy quad", QUAD_FIELDS),
    ("face attribute", FACE_FIELDS),
    ("model vertex", MODEL_FIELDS),
];

#[cfg(test)]
const FLOATS: &[(&str, f32)] = &[
    ("SECTION_SIZE", mcrs_minecraft_network::columns::SECTION_SIZE as f32),
    ("MODEL_OVERHANG", MODEL_OVERHANG),
    ("MODEL_STEPS", MODEL_STEPS),
    ("FLUID_INSET", FLUID_INSET),
];

#[cfg(test)]
const COUNTS: &[(&str, u32)] = &[("QUAD_WORDS", QUAD_WORDS as u32), ("FACE_NONE", FACE_NONE)];

#[cfg(test)]
fn wgsl_fields() -> String {
    let mut out = String::from(
        "// Generated from the field table in pack.rs. `cargo test -p \
mcrs_minecraft_client`\n// checks it; `MCRS_BLESS=1 cargo test -p mcrs_minecraft_client` \
rewrites it.\n#define_import_path mcrs_minecraft_client::fields\n",
    );
    let mut group = "";
    for (name, field) in GROUPS.iter().flat_map(|(_, fields)| fields.iter()) {
        let prefix = name.split_once('_').map_or(*name, |(head, _)| head);
        if prefix != group {
            out.push('\n');
            group = prefix;
        }
        out.push_str(&format!("const {name}_WORD: u32 = {}u;\n", field.word));
        out.push_str(&format!("const {name}_SHIFT: u32 = {}u;\n", field.shift));
        out.push_str(&format!("const {name}_BITS: u32 = {}u;\n", field.bits));
    }
    out.push('\n');
    for (name, value) in COUNTS {
        out.push_str(&format!("const {name}: u32 = {value}u;\n"));
    }
    out.push('\n');
    for (name, value) in FLOATS {
        out.push_str(&format!("const {name}: f32 = {value:?};\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_network::columns::SECTION_SIZE;

    #[test]
    fn the_generated_field_header_matches_the_field_table() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/render/shaders/include/fields.wgsl");
        let generated = wgsl_fields();
        if std::env::var("MCRS_BLESS").is_ok() {
            std::fs::write(&path, &generated).expect("cannot rewrite the generated header");
            return;
        }
        let checked_in = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            checked_in,
            generated,
            "{} is stale; rerun with MCRS_BLESS=1 to rewrite it",
            path.display()
        );
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
        for word in 0..QUAD_WORDS as u32 {
            let bits: u32 = QUAD_FIELDS
                .iter()
                .filter(|(_, field)| field.word == word)
                .map(|(_, field)| field.bits)
                .sum();
            assert!(bits <= 32, "word {word} of a quad holds {bits} bits");
        }
        assert!(
            QUAD_FIELDS
                .iter()
                .all(|(_, field)| (field.word as usize) < QUAD_WORDS)
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
        for (value, fields) in GROUPS {
            let mut taken = [0u64; 4];
            for (_, field) in *fields {
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
