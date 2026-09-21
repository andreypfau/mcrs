use bevy::color::{ColorToPacked, Hsva, Srgba};
use bevy::math::{IRect, IVec2};

use super::scene::GuiQuad;

pub const WHITE: u32 = 0xFFFF_FFFF;
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

pub fn text_width(text: &str, digit_widths: &[u8; 10]) -> i32 {
    text.bytes()
        .map(|b| digit_widths[usize::from(b - b'0')] as i32 + 1)
        .sum()
}

pub fn shadow_color(color: u32) -> u32 {
    (color & 0xFF00_0000) | ((color & 0x00FC_FCFC) >> 2)
}

/// Digits laid out left to right from `origin`, shadow glyph before each glyph.
pub fn glyphs(
    origin: IVec2,
    text: &str,
    color: u32,
    digit_widths: &[u8; 10],
    out: &mut Vec<GuiQuad>,
) {
    let mut x = origin.x;
    for digit in text.bytes().map(|b| b - b'0') {
        out.push(GuiQuad::Glyph {
            origin: IVec2::new(x + 1, origin.y + 1),
            digit,
            color: shadow_color(color),
        });
        out.push(GuiQuad::Glyph {
            origin: IVec2::new(x, origin.y),
            digit,
            color,
        });
        x += digit_widths[usize::from(digit)] as i32 + 1;
    }
}

pub struct Decorated {
    pub count: u8,
    pub damage: Option<(i32, i32)>,
}

pub fn decorations(
    origin: IVec2,
    stack: &Decorated,
    digit_widths: &[u8; 10],
    out: &mut Vec<GuiQuad>,
) {
    if let Some((damage, max_damage)) = stack.damage {
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
    if stack.count != 1 {
        let text = stack.count.to_string();
        let x = origin.x + 19 - 2 - text_width(&text, digit_widths);
        glyphs(
            IVec2::new(x, origin.y + 6 + 3),
            &text,
            WHITE,
            digit_widths,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VANILLA_DIGITS: [u8; 10] = [5; 10];

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

    #[test]
    fn an_undamaged_stack_shows_no_bar() {
        let mut out = Vec::new();
        let stack = Decorated {
            count: 1,
            damage: None,
        };
        decorations(IVec2::ZERO, &stack, &VANILLA_DIGITS, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn the_bar_is_two_fills_at_the_slot_bottom() {
        let mut out = Vec::new();
        let stack = Decorated {
            count: 1,
            damage: Some((50, 100)),
        };
        decorations(IVec2::new(10, 20), &stack, &VANILLA_DIGITS, &mut out);
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

    #[test]
    fn counts_are_right_aligned_with_a_shadow() {
        let mut out = Vec::new();
        let stack = Decorated {
            count: 64,
            damage: None,
        };
        decorations(IVec2::ZERO, &stack, &VANILLA_DIGITS, &mut out);
        assert_eq!(
            out,
            vec![
                GuiQuad::Glyph {
                    origin: IVec2::new(6, 10),
                    digit: 6,
                    color: 0xFF3F_3F3F
                },
                GuiQuad::Glyph {
                    origin: IVec2::new(5, 9),
                    digit: 6,
                    color: WHITE
                },
                GuiQuad::Glyph {
                    origin: IVec2::new(12, 10),
                    digit: 4,
                    color: 0xFF3F_3F3F
                },
                GuiQuad::Glyph {
                    origin: IVec2::new(11, 9),
                    digit: 4,
                    color: WHITE
                },
            ]
        );
        out.clear();
        let seven = Decorated {
            count: 7,
            damage: None,
        };
        decorations(IVec2::ZERO, &seven, &VANILLA_DIGITS, &mut out);
        assert_eq!(out.len(), 2);
        assert!(
            matches!(out[1], GuiQuad::Glyph { origin, digit: 7, .. } if origin == IVec2::new(11, 9))
        );
    }
}
