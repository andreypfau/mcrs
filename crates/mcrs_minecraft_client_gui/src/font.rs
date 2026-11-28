use bevy::ecs::resource::Resource;
use bevy::math::IVec2;

use crate::event_index::EventIndex;
use crate::font_definition::{BitmapPage, FontOptions, Kind, Loading, Source};
use crate::font_error::FontError;
use crate::font_glyph::{Glyph, ImageSource, MISSING};
use crate::font_unihex::CELL_BYTES;
use crate::inflated_file::InflatedFile;
use crate::loaded_font::LoadedFont;
use mcrs_minecraft_resource_pack::PackFiles;

const DEFAULT_FONT: &str = "minecraft:default";

#[derive(Resource, Clone, Debug, Default)]
pub struct Font {
    sources: Vec<Source>,
    options: FontOptions,
}

impl Font {
    pub fn load(
        pack: &PackFiles,
        options: FontOptions,
        inflate: impl Fn(&[u8]) -> Result<Vec<InflatedFile>, String>,
    ) -> Result<LoadedFont, FontError> {
        let mut loading = Loading::new(pack, inflate);
        let sources = loading.sources(DEFAULT_FONT)?;
        Ok(LoadedFont {
            font: Self { sources, options },
            notices: loading.into_notices(),
        })
    }

    #[cfg(test)]
    pub(crate) fn options(&self) -> FontOptions {
        self.options
    }

    fn active(&self) -> impl Iterator<Item = (usize, &Source)> {
        self.sources
            .iter()
            .enumerate()
            .filter(|(_, source)| source.filter.passes(self.options))
    }

    pub fn bitmap_pages(&self) -> impl Iterator<Item = (usize, &BitmapPage)> {
        self.sources
            .iter()
            .enumerate()
            .filter_map(|(index, source)| match &source.kind {
                Kind::Bitmap(page) => Some((index, page)),
                Kind::Space(_) | Kind::Unihex(_) => None,
            })
    }

    #[cfg(test)]
    pub(crate) fn unifonts(&self) -> impl Iterator<Item = (usize, &crate::font_unihex::Unihex)> {
        self.sources
            .iter()
            .enumerate()
            .filter_map(|(index, source)| match &source.kind {
                Kind::Unihex(hex) => Some((index, hex)),
                Kind::Space(_) | Kind::Bitmap(_) => None,
            })
    }

    pub fn glyph(&self, ch: char) -> Glyph {
        self.active()
            .find_map(|(index, source)| match &source.kind {
                Kind::Space(advances) => advances.get(&ch).map(|&advance| Glyph {
                    advance,
                    offset: 1.0,
                    image: None,
                }),
                Kind::Bitmap(page) => page.glyph(index, ch),
                Kind::Unihex(hex) => hex.glyph(index, ch),
            })
            .unwrap_or(MISSING)
    }

    pub fn unifont_cell(&self, ch: char) -> Option<[u8; CELL_BYTES]> {
        let ImageSource::Unifont { source, .. } = self.glyph(ch).image?.source else {
            return None;
        };
        let Kind::Unihex(hex) = &self.sources.get(source)?.kind else {
            return None;
        };
        let mut cell = [0; CELL_BYTES];
        hex.rasterise(ch, &mut cell).then_some(cell)
    }

    pub fn advance(&self, ch: char, bold: bool) -> f32 {
        let glyph = self.glyph(ch);
        glyph.advance + if bold { glyph.offset } else { 0.0 }
    }

    pub fn exact_width(&self, spans: &[Span]) -> f32 {
        spans
            .iter()
            .flat_map(|span| span.text.chars().map(|ch| self.advance(ch, span.bold)))
            .sum()
    }

    pub fn width(&self, spans: &[Span]) -> i32 {
        self.exact_width(spans).ceil() as i32
    }

    pub fn exact_str_width(&self, text: &str) -> f32 {
        styled(text, false)
            .map(|(ch, bold)| self.advance(ch, bold))
            .sum()
    }

    pub fn str_width(&self, text: &str) -> i32 {
        self.exact_str_width(text).ceil() as i32
    }
}

pub(crate) fn styled(text: &str, mut bold: bool) -> impl Iterator<Item = (char, bool)> + '_ {
    let mut chars = text.chars().peekable();
    std::iter::from_fn(move || {
        loop {
            let ch = chars.next()?;
            if ch == '§'
                && let Some(style) = chars.peek().copied().and_then(formatting)
            {
                chars.next();
                bold = match style {
                    Format::Bold => true,
                    Format::Reset => false,
                    Format::Keep => bold,
                };
                continue;
            }
            return Some((ch, bold));
        }
    })
}

enum Format {
    Bold,
    Reset,
    Keep,
}

fn formatting(code: char) -> Option<Format> {
    match code.to_ascii_lowercase() {
        'l' => Some(Format::Bold),
        '0'..='9' | 'a'..='f' | 'r' => Some(Format::Reset),
        'k' | 'm' | 'n' | 'o' => Some(Format::Keep),
        _ => None,
    }
}

pub type ClickEvent = mcrs_minecraft_text::ClickEvent<
    mcrs_minecraft_item::Template<mcrs_minecraft_registries::Named>,
>;

pub type Hover = mcrs_minecraft_text::HoverEvent<
    mcrs_minecraft_item::Template<mcrs_minecraft_registries::Named>,
>;

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: u32,
    pub bold: bool,
    pub hover: Option<EventIndex>,
    pub click: Option<EventIndex>,
    pub insertion: Option<Box<str>>,
}

impl Span {
    pub fn new(text: impl Into<String>, color: u32) -> Self {
        Self {
            text: text.into(),
            color,
            bold: false,
            hover: None,
            click: None,
            insertion: None,
        }
    }

    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }
}

pub fn span_at<'s>(
    font: &Font,
    origin: IVec2,
    spans: &'s [Span],
    point: IVec2,
) -> Option<&'s Span> {
    if point.y < origin.y || point.y >= origin.y + 9 {
        return None;
    }
    let mut x = origin.x as f32;
    for span in spans {
        for ch in span.text.chars() {
            let right = x + font.advance(ch, span.bold);
            if x <= point.x as f32 && (point.x as f32) < right {
                return Some(span);
            }
            x = right;
        }
    }
    None
}

#[cfg(test)]
pub(crate) fn corpus_font() -> &'static Font {
    static FONT: std::sync::LazyLock<Font> = std::sync::LazyLock::new(|| {
        Font::load(
            crate::corpus_pack::corpus_pack(),
            FontOptions::default(),
            crate::inflate_zip::inflate_zip,
        )
        .unwrap()
        .font
    });
    &FONT
}
