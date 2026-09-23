use std::collections::HashMap;

use bevy::math::IVec2;
use serde::Deserialize;

use super::scene::GuiQuad;
use crate::model::{Pack, split_id};

pub const ASCII_TEXTURE: &str = "minecraft/textures/font/ascii.png";
const ASCII_FILE: &str = "minecraft:font/ascii.png";
const DEFINITIONS: [&str; 2] = [
    "minecraft/font/include/space.json",
    "minecraft/font/include/default.json",
];

#[derive(Deserialize)]
struct FontDefinition {
    providers: Vec<Provider>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Provider {
    Bitmap {
        file: String,
        chars: Vec<String>,
    },
    Space {
        advances: HashMap<char, i32>,
    },
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub cell: IVec2,
    pub advance: i32,
}

/// The glyphs of the ASCII page the GUI atlas holds, and the space advances.
#[derive(Debug, Default)]
pub struct Font {
    pub cell_size: IVec2,
    glyphs: HashMap<char, Glyph>,
    spaces: HashMap<char, i32>,
}

impl Font {
    pub fn load(pack: &Pack, ascii: &[u8], width: u32, height: u32) -> Result<Self, String> {
        let mut font = Font::default();
        for path in DEFINITIONS {
            let definition: FontDefinition = serde_json::from_slice(pack.read(path)?)
                .map_err(|error| format!("{path}: {error}"))?;
            for provider in definition.providers {
                match provider {
                    Provider::Space { advances } => font.spaces.extend(advances),
                    Provider::Bitmap { file, chars } if normalized(&file) == ASCII_FILE => {
                        font.load_page(&chars, ascii, width, height)?;
                    }
                    Provider::Bitmap { .. } | Provider::Other => {}
                }
            }
        }
        if font.glyphs.is_empty() {
            return Err(format!("no provider draws from {ASCII_FILE}"));
        }
        Ok(font)
    }

    fn load_page(
        &mut self,
        rows: &[String],
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let columns = rows.first().map_or(0, |row| row.chars().count()) as u32;
        if columns == 0 || width % columns != 0 || height % rows.len() as u32 != 0 {
            return Err(format!("{ASCII_FILE} does not divide into its character grid"));
        }
        self.cell_size = IVec2::new((width / columns) as i32, (height / rows.len() as u32) as i32);
        for (row, line) in rows.iter().enumerate() {
            for (column, ch) in line.chars().enumerate() {
                if ch == '\0' {
                    continue;
                }
                let cell = IVec2::new(column as i32, row as i32) * self.cell_size;
                let inked = (0..self.cell_size.x).rev().find(|&x| {
                    (0..self.cell_size.y).any(|y| {
                        let at = (cell.y + y) as usize * width as usize + (cell.x + x) as usize;
                        pixels[at * 4 + 3] != 0
                    })
                });
                let advance = inked.map_or(0, |x| x + 1) + 1;
                self.glyphs.insert(ch, Glyph { cell, advance });
            }
        }
        Ok(())
    }

    pub fn glyph(&self, ch: char) -> Option<Glyph> {
        self.glyphs.get(&ch).copied()
    }

    pub fn advance(&self, ch: char, bold: bool) -> i32 {
        if let Some(&space) = self.spaces.get(&ch) {
            return space;
        }
        self.glyph(ch)
            .map_or(0, |glyph| glyph.advance + i32::from(bold))
    }

    pub fn width(&self, spans: &[Span]) -> i32 {
        spans
            .iter()
            .flat_map(|span| span.text.chars().map(|ch| self.advance(ch, span.bold)))
            .sum()
    }
}

fn normalized(id: &str) -> String {
    let (namespace, path) = split_id(id);
    format!("{namespace}:{path}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub color: u32,
    pub bold: bool,
}

impl Span {
    pub fn new(text: impl Into<String>, color: u32) -> Self {
        Self {
            text: text.into(),
            color,
            bold: false,
        }
    }

    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }
}

pub fn shadow_color(color: u32) -> u32 {
    (color & 0xFF00_0000) | ((color & 0x00FC_FCFC) >> 2)
}

/// Left to right from `origin`, a shadow one pixel down and right under every
/// glyph; bold strikes each glyph again one pixel to the right.
pub fn draw_text(font: &Font, origin: IVec2, spans: &[Span], out: &mut Vec<GuiQuad>) {
    let mut x = origin.x;
    for span in spans {
        for ch in span.text.chars() {
            if font.glyph(ch).is_some() && !font.spaces.contains_key(&ch) {
                let strikes = if span.bold { 2 } else { 1 };
                for (offset, color) in [(1, shadow_color(span.color)), (0, span.color)] {
                    for strike in 0..strikes {
                        out.push(GuiQuad::Glyph {
                            origin: IVec2::new(x + offset + strike, origin.y + offset),
                            glyph: ch,
                            color,
                        });
                    }
                }
            }
            x += font.advance(ch, span.bold);
        }
    }
}

pub fn draw_centered_text(
    font: &Font,
    center_x: i32,
    y: i32,
    spans: &[Span],
    out: &mut Vec<GuiQuad>,
) {
    let origin = IVec2::new(center_x - font.width(spans) / 2, y);
    draw_text(font, origin, spans, out);
}

#[cfg(test)]
pub(crate) fn corpus_font() -> &'static Font {
    static FONT: std::sync::LazyLock<Font> = std::sync::LazyLock::new(|| {
        let bytes = Pack::corpus().read(ASCII_TEXTURE).unwrap();
        let (pixels, width, height) = crate::atlas::decode_png(bytes, ASCII_TEXTURE).unwrap();
        Font::load(Pack::corpus(), &pixels, width, height).unwrap()
    });
    &FONT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ascii_page_measures_like_vanilla() {
        let font = corpus_font();
        assert_eq!(font.cell_size, IVec2::splat(8));
        assert_eq!(font.advance('0', false), 6);
        assert_eq!(font.advance('i', false), 2);
        assert_eq!(font.advance('l', false), 3);
        assert_eq!(font.advance(' ', false), 4);
        assert_eq!(font.advance('A', true), 7);
        assert_eq!(font.glyph('A').unwrap().cell, IVec2::new(8, 32));
        assert_eq!(font.width(&[Span::new("F4", 0)]), 12);
    }

    #[test]
    fn a_space_draws_nothing_and_bold_strikes_twice() {
        let mut out = Vec::new();
        draw_text(
            corpus_font(),
            IVec2::ZERO,
            &[Span::new("a b", 0xFFFF_FFFF), Span::new("c", 0xFFFF_FF55).bold()],
            &mut out,
        );
        let glyphs: Vec<_> = out
            .iter()
            .map(|quad| match quad {
                GuiQuad::Glyph { origin, glyph, .. } => (*glyph, origin.x),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            glyphs,
            [
                ('a', 1),
                ('a', 0),
                ('b', 11),
                ('b', 10),
                ('c', 17),
                ('c', 18),
                ('c', 16),
                ('c', 17),
            ]
        );
    }
}
