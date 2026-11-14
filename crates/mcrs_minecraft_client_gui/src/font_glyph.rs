use bevy::math::{Rect, Vec2};

/// The growth of the box of a bold strike on each side.
const BOLD_GROWTH: f32 = 0.1;

/// What a font gives for a code point. The bold strike and the shadow share one offset in every
/// vanilla provider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub advance: f32,
    pub offset: f32,
    pub image: Option<GlyphImage>,
}

/// The picture of a glyph and its box in GUI units from the pen position.
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
    /// A cell of the texture of the bitmap provider at index `source`, in texels.
    Bitmap {
        source: usize,
        texels: Rect,
    },
    /// Columns of a Unifont glyph, drawn from the glyph cache.
    Unifont {
        source: usize,
        columns: u32,
    },
    Missing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Weight {
    #[default]
    Regular,
    Bold,
}

impl GlyphImage {
    /// The box of one strike with the pen at `pen`.
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

/// The missing glyph as RGBA pixels, 5 × 8.
pub fn missing_glyph_pixels() -> Vec<u8> {
    MISSING_ROWS
        .iter()
        .flat_map(|row| row.bytes())
        .flat_map(|cell| [255, 255, 255, if cell == b'#' { 255 } else { 0 }])
        .collect()
}

pub const MISSING_GLYPH_SIZE: (u32, u32) = (5, 8);
