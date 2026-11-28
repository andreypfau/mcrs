use bevy::math::{IVec2, Vec2};

use crate::gui_quad::GuiQuad;
use mcrs_minecraft_font::font::{Font, Span, styled};
use mcrs_minecraft_font::font_glyph::Weight;

pub fn shadow_color(color: u32) -> u32 {
    (color & 0xFF00_0000) | ((color & 0x00FC_FCFC) >> 2)
}

pub fn draw_text(font: &Font, origin: IVec2, spans: &[Span], out: &mut Vec<GuiQuad>) {
    draw_tinted_text(font, origin, spans, |color| color, out);
}

pub fn draw_tinted_text(
    font: &Font,
    origin: IVec2,
    spans: &[Span],
    tint: impl Fn(u32) -> u32,
    out: &mut Vec<GuiQuad>,
) {
    draw_spans(font, origin, spans, tint, Shadow::Drawn, out);
}

pub(crate) fn draw_text_without_shadow(
    font: &Font,
    origin: IVec2,
    spans: &[Span],
    out: &mut Vec<GuiQuad>,
) {
    draw_spans(font, origin, spans, |color| color, Shadow::Omitted, out);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shadow {
    Drawn,
    Omitted,
}

fn draw_spans(
    font: &Font,
    origin: IVec2,
    spans: &[Span],
    tint: impl Fn(u32) -> u32,
    shadow: Shadow,
    out: &mut Vec<GuiQuad>,
) {
    let mut x = origin.x as f32;
    for span in spans {
        x = draw_glyphs(
            font,
            Vec2::new(x, origin.y as f32),
            span.text.chars().map(|ch| (ch, span.bold)),
            tint(span.color),
            shadow,
            out,
        );
    }
}

pub fn draw_str(
    font: &Font,
    origin: Vec2,
    text: &str,
    color: u32,
    bold: bool,
    out: &mut Vec<GuiQuad>,
) -> f32 {
    draw_glyphs(font, origin, styled(text, bold), color, Shadow::Drawn, out)
}

pub(crate) fn draw_str_without_shadow(
    font: &Font,
    origin: Vec2,
    text: &str,
    color: u32,
    out: &mut Vec<GuiQuad>,
) {
    draw_glyphs(
        font,
        origin,
        styled(text, false),
        color,
        Shadow::Omitted,
        out,
    );
}

pub(crate) fn draw_glyphs(
    font: &Font,
    origin: Vec2,
    glyphs: impl Iterator<Item = (char, bool)>,
    color: u32,
    shadow: Shadow,
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
            let under = (shadow == Shadow::Drawn).then(|| (glyph.offset, shadow_color(color)));
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
mod tests {
    use super::*;
    use crate::corpus_font::corpus_font;

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
