#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureChoice {
    Named(usize),
    Identifier(u64),
}

pub const MAX_PART_PATTERNS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerChoice {
    pub entry: usize,
    pub geometry: usize,
    pub texture: TextureChoice,
    pub shown_parts: u64,
}

impl LayerChoice {
    #[must_use]
    pub fn shows_part(&self, index: usize) -> bool {
        index < MAX_PART_PATTERNS && self.shown_parts & (1 << index) != 0
    }
}
