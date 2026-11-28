use bevy::math::{IRect, IVec2, Rect};

use crate::gui_quad::GuiQuad;
use crate::gui_quad::WHITE;
use crate::sprite::GuiSprite;
use crate::sprite::Scaling;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PartFill {
    Stretch,
    Tile,
}

pub fn draw_sprite(sprite: &GuiSprite, target: IRect, out: &mut Vec<GuiQuad>) {
    match sprite.scaling {
        Scaling::Stretch => {
            if target.width() > 0 && target.height() > 0 {
                out.push(GuiQuad::Texels {
                    rect: target,
                    texels: sprite.texels,
                    color: WHITE,
                });
            }
        }
        Scaling::Tile { size } => {
            fill_part(
                sprite,
                size,
                target,
                IRect::from_corners(IVec2::ZERO, size),
                PartFill::Tile,
                out,
            );
        }
        Scaling::NineSlice {
            size,
            border,
            stretch_inner,
        } => {
            let whole = target.size();
            let cuts = |low: i32, high: i32, whole: i32, sprite: i32| {
                let (low, high) = (low.min(whole / 2), high.min(whole / 2));
                [
                    (0, low, 0, low),
                    (low, whole - high, low, sprite - high),
                    (whole - high, whole, sprite - high, sprite),
                ]
            };
            let columns = cuts(border.left, border.right, whole.x, size.x);
            let rows = cuts(border.top, border.bottom, whole.y, size.y);
            for (column, (x0, x1, u0, u1)) in columns.into_iter().enumerate() {
                for (row, (y0, y1, v0, v1)) in rows.into_iter().enumerate() {
                    let area = IRect::new(x0, y0, x1, y1);
                    let fill = if (column == 1 || row == 1) && stretch_inner {
                        PartFill::Stretch
                    } else {
                        PartFill::Tile
                    };
                    fill_part(
                        sprite,
                        size,
                        IRect::from_corners(target.min + area.min, target.min + area.max),
                        IRect::new(u0, v0, u1, v1),
                        fill,
                        out,
                    );
                }
            }
        }
    }
}

fn fill_part(
    sprite: &GuiSprite,
    size: IVec2,
    area: IRect,
    part: IRect,
    fill: PartFill,
    out: &mut Vec<GuiQuad>,
) {
    if area.width() <= 0 || area.height() <= 0 || part.width() <= 0 || part.height() <= 0 {
        return;
    }
    if fill == PartFill::Stretch {
        out.push(part_quad(sprite, size, area, part));
        return;
    }
    for y in (area.min.y..area.max.y).step_by(part.height() as usize) {
        for x in (area.min.x..area.max.x).step_by(part.width() as usize) {
            let at = IVec2::new(x, y);
            let tile = (area.max - at).min(part.size());
            out.push(part_quad(
                sprite,
                size,
                IRect::from_corners(at, at + tile),
                IRect::from_corners(part.min, part.min + tile),
            ));
        }
    }
}

fn part_quad(sprite: &GuiSprite, size: IVec2, rect: IRect, part: IRect) -> GuiQuad {
    let scale = sprite.texels.size() / size.as_vec2();
    let texel = |at: IVec2| sprite.texels.min + at.as_vec2() * scale;
    GuiQuad::Texels {
        rect,
        texels: Rect::from_corners(texel(part.min), texel(part.max)),
        color: WHITE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_sprite_id::GuiSpriteId;
    use crate::menu_test_app::ATLAS;
    use crate::sprite::Border;

    type Quad = ([i32; 4], [i32; 4]);

    fn sorted_quads(sprite: GuiSprite, target: IRect) -> Vec<Quad> {
        let corners = |r: IRect| [r.min.x, r.min.y, r.max.x, r.max.y];
        let mut out = Vec::new();
        draw_sprite(&sprite, target, &mut out);
        let mut quads: Vec<Quad> = out
            .into_iter()
            .filter_map(|quad| match quad {
                GuiQuad::Texels { rect, texels, .. } => {
                    Some((corners(rect), corners(texels.as_irect())))
                }
                _ => None,
            })
            .collect();
        quads.sort();
        quads
    }

    #[test]
    fn a_tile_is_cut_at_the_far_edge_and_an_empty_target_draws_nothing() {
        let cases: [(&str, GuiSprite, IRect, Vec<Quad>); 2] = [
            (
                "tile cut at the right edge",
                GuiSprite::new(
                    IRect::new(0, 0, 16, 16),
                    Scaling::Tile {
                        size: IVec2::splat(16),
                    },
                ),
                IRect::new(0, 0, 40, 16),
                vec![
                    ([0, 0, 16, 16], [0, 0, 16, 16]),
                    ([16, 0, 32, 16], [0, 0, 16, 16]),
                    ([32, 0, 40, 16], [0, 0, 8, 16]),
                ],
            ),
            (
                "stretch into a target of width 0",
                GuiSprite::new(IRect::new(0, 0, 16, 16), Scaling::Stretch),
                IRect::new(5, 5, 5, 40),
                vec![],
            ),
        ];
        for (name, sprite, target, mut expected) in cases {
            expected.sort();
            assert_eq!(sorted_quads(sprite, target), expected, "{name}");
        }
    }

    #[test]
    fn a_nine_slice_fills_the_target_with_these_quads() {
        let nine = |atlas: IRect, border, stretch_inner| {
            GuiSprite::new(
                atlas,
                Scaling::NineSlice {
                    size: IVec2::splat(10),
                    border,
                    stretch_inner,
                },
            )
        };
        let uneven = Border {
            left: 2,
            top: 3,
            right: 4,
            bottom: 1,
        };
        let cases: [(&str, GuiSprite, IRect, Vec<Quad>); 4] = [
            (
                "nine slice, inner parts stretch",
                nine(IRect::new(0, 0, 10, 10), uneven, true),
                IRect::new(100, 100, 120, 106),
                vec![
                    ([100, 100, 102, 103], [0, 0, 2, 3]),
                    ([100, 103, 102, 105], [0, 3, 2, 9]),
                    ([100, 105, 102, 106], [0, 9, 2, 10]),
                    ([102, 100, 116, 103], [2, 0, 6, 3]),
                    ([102, 103, 116, 105], [2, 3, 6, 9]),
                    ([102, 105, 116, 106], [2, 9, 6, 10]),
                    ([116, 100, 120, 103], [6, 0, 10, 3]),
                    ([116, 103, 120, 105], [6, 3, 10, 9]),
                    ([116, 105, 120, 106], [6, 9, 10, 10]),
                ],
            ),
            (
                "nine slice at an atlas offset",
                nine(IRect::new(32, 48, 42, 58), uneven, true),
                IRect::new(0, 0, 10, 10),
                vec![
                    ([0, 0, 2, 3], [32, 48, 34, 51]),
                    ([0, 3, 2, 9], [32, 51, 34, 57]),
                    ([0, 9, 2, 10], [32, 57, 34, 58]),
                    ([2, 0, 6, 3], [34, 48, 38, 51]),
                    ([2, 3, 6, 9], [34, 51, 38, 57]),
                    ([2, 9, 6, 10], [34, 57, 38, 58]),
                    ([6, 0, 10, 3], [38, 48, 42, 51]),
                    ([6, 3, 10, 9], [38, 51, 42, 57]),
                    ([6, 9, 10, 10], [38, 57, 42, 58]),
                ],
            ),
            (
                "nine slice borders reduced to half the target",
                nine(IRect::new(0, 0, 10, 10), uneven, true),
                IRect::new(0, 0, 5, 3),
                vec![
                    ([0, 0, 2, 1], [0, 0, 2, 1]),
                    ([0, 1, 2, 2], [0, 1, 2, 9]),
                    ([0, 2, 2, 3], [0, 9, 2, 10]),
                    ([2, 0, 3, 1], [2, 0, 8, 1]),
                    ([2, 1, 3, 2], [2, 1, 8, 9]),
                    ([2, 2, 3, 3], [2, 9, 8, 10]),
                    ([3, 0, 5, 1], [8, 0, 10, 1]),
                    ([3, 1, 5, 2], [8, 1, 10, 9]),
                    ([3, 2, 5, 3], [8, 9, 10, 10]),
                ],
            ),
            (
                "nine slice, inner parts tile from their top left corner",
                nine(IRect::new(0, 0, 10, 10), Border::splat(2), false),
                IRect::new(0, 0, 15, 10),
                vec![
                    ([0, 0, 2, 2], [0, 0, 2, 2]),
                    ([0, 2, 2, 8], [0, 2, 2, 8]),
                    ([0, 8, 2, 10], [0, 8, 2, 10]),
                    ([2, 0, 8, 2], [2, 0, 8, 2]),
                    ([2, 2, 8, 8], [2, 2, 8, 8]),
                    ([2, 8, 8, 10], [2, 8, 8, 10]),
                    ([8, 0, 13, 2], [2, 0, 7, 2]),
                    ([8, 2, 13, 8], [2, 2, 7, 8]),
                    ([8, 8, 13, 10], [2, 8, 7, 10]),
                    ([13, 0, 15, 2], [8, 0, 10, 2]),
                    ([13, 2, 15, 8], [8, 2, 10, 8]),
                    ([13, 8, 15, 10], [8, 8, 10, 10]),
                ],
            ),
        ];
        for (name, sprite, target, mut expected) in cases {
            expected.sort();
            assert_eq!(sorted_quads(sprite, target), expected, "{name}");
        }
    }

    #[test]
    fn the_button_sprite_of_the_pack_draws_as_nine_slice_quads_from_its_region() {
        let atlas = &ATLAS.layout;
        let button = atlas.region(GuiSpriteId::Button.name()).min;
        let mut out = Vec::new();
        draw_sprite(
            &atlas.sprite(GuiSpriteId::Button),
            IRect::new(0, 0, 50, 20),
            &mut out,
        );
        assert_eq!(out.len(), 9, "{out:?}");
        for (at, texel) in [((0, 0), (0, 0)), ((47, 0), (197, 0)), ((0, 17), (0, 17))] {
            let (at, texel) = (IVec2::from(at), button + IVec2::from(texel));
            let corner = GuiQuad::Texels {
                rect: IRect::from_corners(at, at + 3),
                texels: Rect::from_corners(texel.as_vec2(), (texel + 3).as_vec2()),
                color: WHITE,
            };
            assert!(out.contains(&corner), "{corner:?} in {out:?}");
        }
    }
}
