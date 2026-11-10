use std::collections::BTreeMap;

use bevy::math::{Rect, URect, UVec2};
use serde::Deserialize;

use crate::font_definition::Filter;
use crate::font_glyph::{Glyph, GlyphImage, ImageSource};

const DEFAULT_HEIGHT: i32 = 8;
/// The row of the line that a glyph with ascent 7 sits on.
const BASELINE: i32 = 7;

#[derive(Deserialize)]
pub(crate) struct BitmapSpec {
    #[serde(default)]
    pub(crate) filter: Filter,
    pub(crate) file: String,
    #[serde(default = "default_height")]
    height: i32,
    ascent: i32,
    chars: Vec<String>,
}

fn default_height() -> i32 {
    DEFAULT_HEIGHT
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    texels: URect,
    advance: f32,
}

/// One texture of glyph cells and the glyphs its grid names.
#[derive(Debug)]
pub struct BitmapPage {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    scale: f32,
    top: f32,
    cells: BTreeMap<char, Cell>,
}

impl BitmapPage {
    /// `pixels` are the RGBA bytes of the texture of `width` × `height` pixels.
    pub(crate) fn new(
        spec: &BitmapSpec,
        pixels: Vec<u8>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        if spec.ascent > spec.height {
            return Err(format!(
                "{}: the ascent {} is above the height {}",
                spec.file, spec.ascent, spec.height
            ));
        }
        let columns = spec.chars.first().map_or(0, |row| row.chars().count()) as u32;
        let rows = spec.chars.len() as u32;
        if columns == 0
            || spec
                .chars
                .iter()
                .any(|row| row.chars().count() as u32 != columns)
        {
            return Err(format!("{}: the rows of chars differ in length", spec.file));
        }
        let (cell_width, cell_height) = (width / columns, height / rows);
        if cell_width == 0 || cell_height == 0 || spec.height <= 0 {
            return Err(format!(
                "{}: the texture is smaller than its grid",
                spec.file
            ));
        }
        let scale = spec.height as f32 / cell_height as f32;
        let mut cells = BTreeMap::new();
        for (row, line) in spec.chars.iter().enumerate() {
            for (column, ch) in line.chars().enumerate() {
                if ch == '\0' {
                    continue;
                }
                let min = UVec2::new(column as u32 * cell_width, row as u32 * cell_height);
                let texels = URect::from_corners(min, min + UVec2::new(cell_width, cell_height));
                let ink = ink_width(&pixels, width, texels);
                let advance = (0.5 + ink as f32 * scale).floor() + 1.0;
                cells.insert(ch, Cell { texels, advance });
            }
        }
        Ok(Self {
            pixels,
            width,
            height,
            scale,
            top: (BASELINE - spec.ascent) as f32,
            cells,
        })
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The glyph of `ch` in this texture, whose provider has the index `source`.
    pub fn glyph(&self, source: usize, ch: char) -> Option<Glyph> {
        let cell = self.cells.get(&ch)?;
        let texels = cell.texels;
        Some(Glyph {
            advance: cell.advance,
            offset: 1.0,
            image: Some(GlyphImage {
                source: ImageSource::Bitmap {
                    source,
                    texels: Rect::new(
                        texels.min.x as f32,
                        texels.min.y as f32,
                        texels.max.x as f32,
                        texels.max.y as f32,
                    ),
                },
                left: 0.0,
                right: texels.width() as f32 * self.scale,
                top: self.top,
                bottom: self.top + texels.height() as f32 * self.scale,
            }),
        })
    }
}

/// The columns up to and including the rightmost one with ink, counted from the cell's left edge.
fn ink_width(pixels: &[u8], width: u32, cell: URect) -> u32 {
    (0..cell.width())
        .rev()
        .find(|&x| {
            (0..cell.height()).any(|y| {
                let at = (cell.min.y + y) as usize * width as usize + (cell.min.x + x) as usize;
                pixels.get(at * 4 + 3).is_some_and(|alpha| *alpha != 0)
            })
        })
        .map_or(0, |x| x + 1)
}
