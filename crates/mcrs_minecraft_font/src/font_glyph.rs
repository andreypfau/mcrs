use bevy_math::{Rect, Vec2};

const BOLD_GROWTH: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub advance: f32,
    pub offset: f32,
    pub image: Option<GlyphImage>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphImage {
    pub source: ImageSource,
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageSource {
    Bitmap { source: usize, texels: Rect },
    Unifont { source: usize, columns: u32 },
    Missing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Weight {
    #[default]
    Regular,
    Bold,
}

impl GlyphImage {
    pub fn strike(&self, pen: Vec2, weight: Weight) -> Rect {
        let growth = match weight {
            Weight::Regular => 0.0,
            Weight::Bold => BOLD_GROWTH,
        };
        Rect::new(
            pen.x + self.left - growth,
            pen.y + self.top - growth,
            pen.x + self.right + growth,
            pen.y + self.bottom + growth,
        )
    }
}

pub(crate) const MISSING_ROWS: [&str; 8] = [
    "#####", "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", "#####",
];

pub(crate) const MISSING: Glyph = Glyph {
    advance: 6.0,
    offset: 1.0,
    image: Some(GlyphImage {
        source: ImageSource::Missing,
        left: 0.0,
        right: 5.0,
        top: 0.0,
        bottom: 8.0,
    }),
};

pub fn missing_glyph_pixels() -> Vec<u8> {
    MISSING_ROWS
        .iter()
        .flat_map(|row| row.bytes())
        .flat_map(|cell| [255, 255, 255, if cell == b'#' { 255 } else { 0 }])
        .collect()
}

pub const MISSING_GLYPH_SIZE: (u32, u32) = (5, 8);
