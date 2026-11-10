use bevy::math::{IVec2, Vec2};

use crate::font_bitmap::BitmapPage;
use crate::font_definition::{FontOptions, Kind, Source, sources};
use crate::font_glyph::{Glyph, ImageSource, MISSING, Weight};
use crate::font_unihex::CELL_BYTES;
use crate::scene::GuiQuad;
use mcrs_minecraft_model::resource_set::ResourceSet;

const DEFAULT_FONT: &str = "minecraft:default";

/// The glyphs of the default font: every provider of its definition, in search order.
#[derive(Debug, Default)]
pub struct Font {
    sources: Vec<Source>,
    options: FontOptions,
}

impl Font {
    /// Loads `minecraft:default` from `pack`.
    // chisle: the pack holds one file per path, so a font with several definition files (a
    // resource pack over the client) is not stacked; the client has no extra packs yet.
    pub fn load(pack: &ResourceSet, options: FontOptions) -> Result<Self, String> {
        Ok(Self {
            sources: sources(pack, DEFAULT_FONT)?,
            options,
        })
    }

    #[cfg(test)]
    pub(crate) fn options(&self) -> FontOptions {
        self.options
    }

    /// The glyph sources that pass the options, in the order the client asks them, with their
    /// index among all the sources.
    fn active(&self) -> impl Iterator<Item = (usize, &Source)> {
        self.sources
            .iter()
            .enumerate()
            .filter(|(_, source)| source.filter.passes(self.options))
    }

    /// The textures of the bitmap providers, by the index of their provider.
    pub fn bitmap_pages(&self) -> impl Iterator<Item = (usize, &BitmapPage)> {
        self.sources
            .iter()
            .enumerate()
            .filter_map(|(index, source)| match &source.kind {
                Kind::Bitmap(page) => Some((index, page)),
                Kind::Space(_) | Kind::Unihex(_) => None,
            })
    }

    /// The Unifont archives of the font, by the index of their provider.
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

    /// The glyph of the first source that has `ch`, or the missing glyph.
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

    /// The pixels of the Unifont glyph that draws `ch`, or `None` when another source draws it.
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

    /// The sum of the advances, before it is rounded up.
    pub fn exact_width(&self, spans: &[Span]) -> f32 {
        spans
            .iter()
            .flat_map(|span| span.text.chars().map(|ch| self.advance(ch, span.bold)))
            .sum()
    }

    pub fn width(&self, spans: &[Span]) -> i32 {
        self.exact_width(spans).ceil() as i32
    }

    /// The width of `text` in the regular weight. A `§` and the formatting character after it
    /// change the style and have no width.
    pub fn exact_str_width(&self, text: &str) -> f32 {
        styled(text, false)
            .map(|(ch, bold)| self.advance(ch, bold))
            .sum()
    }

    pub fn str_width(&self, text: &str) -> i32 {
        self.exact_str_width(text).ceil() as i32
    }
}

/// The code points of a plain string with the weight each is drawn in: a `§` and the formatting
/// character after it change the weight and are not code points. Measuring and drawing a plain
/// string both go through this, so they agree.
fn styled(text: &str, mut bold: bool) -> impl Iterator<Item = (char, bool)> + '_ {
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
    /// A colour or the reset: the style starts again from the plain one.
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

/// The click event of a span.
pub type ClickEvent = mcrs_minecraft_protocol_game::text::ClickEvent<
    mcrs_minecraft_item::Template<mcrs_minecraft_registries::Named>,
>;

/// The hover event of a span.
pub type Hover = mcrs_minecraft_protocol_game::text::HoverEvent<
    mcrs_minecraft_item::Template<mcrs_minecraft_registries::Named>,
>;

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: u32,
    pub bold: bool,
    pub hover: Option<Box<Hover>>,
    pub click: Option<Box<ClickEvent>>,
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

pub fn shadow_color(color: u32) -> u32 {
    (color & 0xFF00_0000) | ((color & 0x00FC_FCFC) >> 2)
}

/// Left to right from `origin`, a shadow one pixel down and right under every
/// glyph; bold strikes each glyph again one pixel to the right.
pub fn draw_text(font: &Font, origin: IVec2, spans: &[Span], out: &mut Vec<GuiQuad>) {
    draw_tinted_text(font, origin, spans, |color| color, out);
}

/// Draws `spans` as [`draw_text`] does, with each colour passed through `tint`.
pub fn draw_tinted_text(
    font: &Font,
    origin: IVec2,
    spans: &[Span],
    tint: impl Fn(u32) -> u32,
    out: &mut Vec<GuiQuad>,
) {
    let mut x = origin.x as f32;
    for span in spans {
        x = draw_glyphs(
            font,
            Vec2::new(x, origin.y as f32),
            span.text.chars().map(|ch| (ch, span.bold)),
            tint(span.color),
            true,
            out,
        );
    }
}

/// Draws a plain string as [`draw_text`] does a span and returns the x where the next run starts.
/// A `§` and its formatting character are skipped and change the weight, as
/// [`Font::exact_str_width`] counts them. Each glyph with a picture is drawn under its shadow, and
/// again one bold offset to the right when bold.
pub fn draw_str(
    font: &Font,
    origin: Vec2,
    text: &str,
    color: u32,
    bold: bool,
    out: &mut Vec<GuiQuad>,
) -> f32 {
    draw_glyphs(font, origin, styled(text, bold), color, true, out)
}

/// Draws one run as [`draw_str`] does a run that is not bold, without the shadow.
pub(crate) fn draw_str_without_shadow(
    font: &Font,
    origin: Vec2,
    text: &str,
    color: u32,
    out: &mut Vec<GuiQuad>,
) {
    draw_glyphs(font, origin, styled(text, false), color, false, out);
}

fn draw_glyphs(
    font: &Font,
    origin: Vec2,
    glyphs: impl Iterator<Item = (char, bool)>,
    color: u32,
    shadow: bool,
    out: &mut Vec<GuiQuad>,
) -> f32 {
    let mut x = origin.x;
    for (ch, bold) in glyphs {
        let (weight, strikes) = if bold {
            (Weight::Bold, 2)
        } else {
            (Weight::Regular, 1)
        };
        let glyph = font.glyph(ch);
        if let Some(image) = glyph.image {
            let under = shadow.then(|| (glyph.offset, shadow_color(color)));
            for (shift, color) in under.into_iter().chain([(0.0, color)]) {
                for strike in 0..strikes {
                    out.push(GuiQuad::Glyph {
                        pen: Vec2::new(x + shift + strike as f32 * glyph.offset, origin.y + shift),
                        glyph: ch,
                        image,
                        color,
                        weight,
                    });
                }
            }
        }
        x += glyph.advance + if bold { glyph.offset } else { 0.0 };
    }
    x
}

/// The span of the glyph whose hover area holds `point`, for text drawn at `origin`. Every glyph
/// covers its advance and 9 units down.
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
        Font::load(ResourceSet::corpus(), FontOptions::default()).unwrap()
    });
    &FONT
}

#[cfg(test)]
mod tests {
    use super::*;

    mod jar {
        use super::*;

        #[test]
        fn a_plain_string_is_drawn_as_wide_as_it_measures() {
            let font = corpus_font();
            for text in ["§lBold§r text", "a§zb", "tail§"] {
                let end = draw_str(font, Vec2::ZERO, text, 0, false, &mut Vec::new());
                assert_eq!(end, font.exact_str_width(text), "{text}");
            }
            assert_eq!(font.exact_str_width("§lA"), font.advance('A', true));
        }

        #[test]
        fn a_space_draws_nothing_and_bold_strikes_twice() {
            let mut out = Vec::new();
            draw_text(
                corpus_font(),
                IVec2::ZERO,
                &[
                    Span::new("a b", 0xFFFF_FFFF),
                    Span::new("c", 0xFFFF_FF55).bold(),
                ],
                &mut out,
            );
            let glyphs: Vec<_> = out
                .iter()
                .map(|quad| match quad {
                    GuiQuad::Glyph { pen, glyph, .. } => (*glyph, pen.x),
                    other => panic!("{other:?}"),
                })
                .collect();
            assert_eq!(
                glyphs,
                [
                    ('a', 1.0),
                    ('a', 0.0),
                    ('b', 11.0),
                    ('b', 10.0),
                    ('c', 17.0),
                    ('c', 18.0),
                    ('c', 16.0),
                    ('c', 17.0),
                ]
            );
        }
    }
}
