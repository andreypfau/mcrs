use bevy::color::{ColorToPacked, Hsva, Srgba};
use bevy::math::{IRect, IVec2};

use crate::font::{Font, Span, draw_text};
use crate::scene::GuiQuad;
use mcrs_minecraft_item_icon::Durability;

pub const WHITE: u32 = 0xFFFF_FFFF;
const CAPPED: u32 = 0xFFFF_FF55;
const BAR_BACK: u32 = 0xFF00_0000;

pub fn bar_width(damage: i32, max_damage: i32) -> i32 {
    let width = 13.0 - damage as f32 * 13.0 / max_damage as f32;
    ((width + 0.5).floor() as i32).clamp(0, 13)
}

pub fn bar_color(damage: i32, max_damage: i32) -> u32 {
    let health = ((max_damage - damage) as f32 / max_damage as f32).max(0.0);
    let [r, g, b, _] = Srgba::from(Hsva::hsv(health / 3.0 * 360.0, 1.0, 1.0)).to_u8_array();
    u32::from_be_bytes([0, r, g, b])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountLabel {
    Plain(u8),
    Capped(u8),
}

pub struct Decorated {
    pub count: CountLabel,
    pub damage: Option<Durability>,
}

pub fn decorations(origin: IVec2, stack: &Decorated, font: &Font, out: &mut Vec<GuiQuad>) {
    if let Some(Durability { damage, max_damage }) = stack.damage {
        let (left, top) = (origin.x + 2, origin.y + 13);
        out.push(GuiQuad::Fill {
            rect: IRect::new(left, top, left + 13, top + 2),
            color: BAR_BACK,
        });
        out.push(GuiQuad::Fill {
            rect: IRect::new(left, top, left + bar_width(damage, max_damage), top + 1),
            color: 0xFF00_0000 | bar_color(damage, max_damage),
        });
    }
    let (count, color) = match stack.count {
        CountLabel::Plain(1) => return,
        CountLabel::Plain(count) => (count, WHITE),
        CountLabel::Capped(count) => (count, CAPPED),
    };
    let text = [Span::new(count.to_string(), color)];
    let x = origin.x + 19 - 2 - font.width(&text);
    draw_text(font, IVec2::new(x, origin.y + 6 + 3), &text, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::corpus_font;
    use crate::font_glyph::Weight;
    use bevy::math::Vec2;

    #[test]
    fn bar_width_and_hue_follow_the_damage() {
        assert_eq!(bar_width(1, 100), 13);
        assert_eq!(bar_width(50, 100), 7);
        assert_eq!(bar_width(99, 100), 0);
        assert_eq!(bar_color(0, 100), 0x00FF00);
        assert_eq!(bar_color(50, 100), 0xFFFF00);
        assert_eq!(bar_color(99, 100), 0xFF0500);
        assert_eq!(bar_color(100, 100), 0xFF0000);
    }

    mod jar {
        use super::*;

        #[test]
        fn decorations_draw_the_damage_bar_and_the_count() {
            an_undamaged_stack_shows_no_bar();
            the_bar_is_two_fills_at_the_slot_bottom();
            counts_are_right_aligned_with_a_shadow();
            a_capped_count_shows_in_yellow_also_at_one();
        }
    }

    fn an_undamaged_stack_shows_no_bar() {
        let mut out = Vec::new();
        let stack = Decorated {
            count: CountLabel::Plain(1),
            damage: None,
        };
        decorations(IVec2::ZERO, &stack, corpus_font(), &mut out);
        assert!(out.is_empty());
    }

    fn the_bar_is_two_fills_at_the_slot_bottom() {
        let mut out = Vec::new();
        let stack = Decorated {
            count: CountLabel::Plain(1),
            damage: Some(Durability {
                damage: 50,
                max_damage: 100,
            }),
        };
        decorations(IVec2::new(10, 20), &stack, corpus_font(), &mut out);
        assert_eq!(
            out,
            vec![
                GuiQuad::Fill {
                    rect: IRect::new(12, 33, 25, 35),
                    color: 0xFF00_0000
                },
                GuiQuad::Fill {
                    rect: IRect::new(12, 33, 19, 34),
                    color: 0xFFFF_FF00
                },
            ]
        );
    }

    fn counts_are_right_aligned_with_a_shadow() {
        let mut out = Vec::new();
        let stack = Decorated {
            count: CountLabel::Plain(64),
            damage: None,
        };
        decorations(IVec2::ZERO, &stack, corpus_font(), &mut out);
        assert_eq!(
            out,
            vec![
                GuiQuad::Glyph {
                    pen: Vec2::new(6.0, 10.0),
                    glyph: '6',
                    image: corpus_font().glyph('6').image.unwrap(),
                    color: 0xFF3F_3F3F,
                    weight: Weight::Regular,
                },
                GuiQuad::Glyph {
                    pen: Vec2::new(5.0, 9.0),
                    glyph: '6',
                    image: corpus_font().glyph('6').image.unwrap(),
                    color: WHITE,
                    weight: Weight::Regular,
                },
                GuiQuad::Glyph {
                    pen: Vec2::new(12.0, 10.0),
                    glyph: '4',
                    image: corpus_font().glyph('4').image.unwrap(),
                    color: 0xFF3F_3F3F,
                    weight: Weight::Regular,
                },
                GuiQuad::Glyph {
                    pen: Vec2::new(11.0, 9.0),
                    glyph: '4',
                    image: corpus_font().glyph('4').image.unwrap(),
                    color: WHITE,
                    weight: Weight::Regular,
                },
            ]
        );
        out.clear();
        let seven = Decorated {
            count: CountLabel::Plain(7),
            damage: None,
        };
        decorations(IVec2::ZERO, &seven, corpus_font(), &mut out);
        assert_eq!(out.len(), 2);
        assert!(
            matches!(out[1], GuiQuad::Glyph { pen, glyph: '7', .. } if pen == Vec2::new(11.0, 9.0))
        );
    }

    fn a_capped_count_shows_in_yellow_also_at_one() {
        let mut out = Vec::new();
        let stack = Decorated {
            count: CountLabel::Capped(1),
            damage: None,
        };
        decorations(IVec2::ZERO, &stack, corpus_font(), &mut out);
        assert_eq!(out.len(), 2, "a shadow and the text of one glyph");
        assert!(
            matches!(
                out[1],
                GuiQuad::Glyph {
                    glyph: '1',
                    color: 0xFFFF_FF55,
                    ..
                }
            ),
            "the yellow text of a capped entry, {:?}",
            out[1]
        );
    }
}
