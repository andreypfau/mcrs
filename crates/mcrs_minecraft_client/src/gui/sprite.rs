use std::fmt;

use bevy::math::{IRect, IVec2, Rect};
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::scene::GuiQuad;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Border {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Border {
    pub fn splat(border: i32) -> Self {
        Self {
            left: border,
            top: border,
            right: border,
            bottom: border,
        }
    }
}

/// How a GUI sprite fills a target (GSPR-META-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scaling {
    Stretch,
    Tile {
        size: IVec2,
    },
    NineSlice {
        size: IVec2,
        border: Border,
        stretch_inner: bool,
    },
}

#[derive(Deserialize)]
struct Meta {
    gui: Option<GuiSection>,
}

#[derive(Deserialize)]
struct GuiSection {
    scaling: Option<ScalingRepr>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ScalingRepr {
    Stretch,
    Tile {
        width: u32,
        height: u32,
    },
    NineSlice {
        width: u32,
        height: u32,
        border: BorderField,
        #[serde(default)]
        stretch_inner: bool,
    },
}

/// A `nine_slice` border: one integer for every side, or an object with each side.
struct BorderField(Border);

impl<'de> Deserialize<'de> for BorderField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BorderVisitor;

        impl<'de> Visitor<'de> for BorderVisitor {
            type Value = BorderField;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an integer or an object with left, top, right and bottom")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<BorderField, E> {
                match i32::try_from(value) {
                    Ok(border) if border > 0 => Ok(BorderField(Border::splat(border))),
                    _ => Err(E::custom(format_args!("border {value} is out of range"))),
                }
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<BorderField, A::Error> {
                Border::deserialize(MapAccessDeserializer::new(map)).map(BorderField)
            }
        }

        deserializer.deserialize_any(BorderVisitor)
    }
}

fn size(width: u32, height: u32) -> Result<IVec2, String> {
    let int = |value: u32| i32::try_from(value).map_err(|_| format!("{value} is too large"));
    let size = IVec2::new(int(width)?, int(height)?);
    if size.min_element() <= 0 {
        return Err("the width and the height must be greater than 0".to_owned());
    }
    Ok(size)
}

impl Scaling {
    /// The scaling in the sprite metadata, or `stretch` without one (GSPR-META-001). An error
    /// stops the load of this sprite only (GSPR-META-002).
    pub fn read(mcmeta: Option<&[u8]>) -> Result<Self, String> {
        let Some(bytes) = mcmeta else {
            return Ok(Self::Stretch);
        };
        let meta: Meta = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let Some(scaling) = meta.gui.and_then(|gui| gui.scaling) else {
            return Ok(Self::Stretch);
        };
        Ok(match scaling {
            ScalingRepr::Stretch => Self::Stretch,
            ScalingRepr::Tile { width, height } => Self::Tile {
                size: size(width, height)?,
            },
            ScalingRepr::NineSlice {
                width,
                height,
                border,
                stretch_inner,
            } => {
                let size = size(width, height)?;
                let BorderField(border) = border;
                let sides = [border.left, border.top, border.right, border.bottom];
                if sides.iter().any(|&side| side < 0) {
                    return Err("a side of the border is less than 0".to_owned());
                }
                let across = i64::from(border.left) + i64::from(border.right);
                let down = i64::from(border.top) + i64::from(border.bottom);
                if across >= i64::from(size.x) || down >= i64::from(size.y) {
                    return Err("the border is as wide or as high as the sprite".to_owned());
                }
                Self::NineSlice {
                    size,
                    border,
                    stretch_inner,
                }
            }
        })
    }
}

/// A GUI sprite placed in the GUI atlas, with its scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuiSprite {
    texels: Rect,
    scaling: Scaling,
}

impl GuiSprite {
    pub fn new(texels: IRect, scaling: Scaling) -> Self {
        Self {
            texels: texels.as_rect(),
            scaling,
        }
    }

    /// Fills `target` by the scaling (GSPR-DRAW-001 to GSPR-DRAW-003).
    pub fn draw(&self, target: IRect, out: &mut Vec<GuiQuad>) {
        match self.scaling {
            Scaling::Stretch => {
                if target.width() > 0 && target.height() > 0 {
                    out.push(GuiQuad::Texels {
                        rect: target,
                        texels: self.texels,
                    });
                }
            }
            Scaling::Tile { size } => {
                self.fill(
                    size,
                    target,
                    IRect::from_corners(IVec2::ZERO, size),
                    false,
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
                        let inner = column == 1 || row == 1;
                        self.fill(
                            size,
                            IRect::from_corners(target.min + area.min, target.min + area.max),
                            IRect::new(u0, v0, u1, v1),
                            inner && stretch_inner,
                            out,
                        );
                    }
                }
            }
        }
    }

    fn fill(&self, size: IVec2, area: IRect, part: IRect, stretch: bool, out: &mut Vec<GuiQuad>) {
        if area.width() <= 0 || area.height() <= 0 || part.width() <= 0 || part.height() <= 0 {
            return;
        }
        if stretch {
            out.push(self.quad(size, area, part));
            return;
        }
        for y in (area.min.y..area.max.y).step_by(part.height() as usize) {
            for x in (area.min.x..area.max.x).step_by(part.width() as usize) {
                let at = IVec2::new(x, y);
                let tile = (area.max - at).min(part.size());
                out.push(self.quad(
                    size,
                    IRect::from_corners(at, at + tile),
                    IRect::from_corners(part.min, part.min + tile),
                ));
            }
        }
    }

    fn quad(&self, size: IVec2, rect: IRect, part: IRect) -> GuiQuad {
        let scale = self.texels.size() / size.as_vec2();
        let texel = |at: IVec2| self.texels.min + at.as_vec2() * scale;
        GuiQuad::Texels {
            rect,
            texels: Rect::from_corners(texel(part.min), texel(part.max)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scaling_defaults_to_stretch_and_rejects_a_bad_gui_section() {
        let sides = |left, top, right, bottom| Border {
            left,
            top,
            right,
            bottom,
        };
        let nine = |border, stretch_inner| {
            Ok(Scaling::NineSlice {
                size: IVec2::new(100, 20),
                border,
                stretch_inner,
            })
        };
        for (json, scaling) in [
            (None, Ok(Scaling::Stretch)),
            (Some(r#"{"animation": {}}"#), Ok(Scaling::Stretch)),
            (Some(r#"{"gui": {"extra": 1}}"#), Ok(Scaling::Stretch)),
            (
                Some(r#"{"gui": {"scaling": {"type": "tile", "width": 16, "height": 8}}}"#),
                Ok(Scaling::Tile {
                    size: IVec2::new(16, 8),
                }),
            ),
            (
                Some(
                    r#"{"gui": {"scaling": {"type": "nine_slice", "width": 100, "height": 20, "border": 9}}}"#,
                ),
                nine(Border::splat(9), false),
            ),
            (
                Some(
                    r#"{"gui": {"scaling": {"type": "nine_slice", "width": 100, "height": 20, "stretch_inner": true,
                    "border": {"left": 20, "top": 0, "right": 79, "bottom": 19}}}}"#,
                ),
                nine(sides(20, 0, 79, 19), true),
            ),
        ] {
            assert_eq!(Scaling::read(json.map(str::as_bytes)), scaling, "{json:?}");
        }
        for json in [
            "{",
            r#"{"gui": {"scaling": {"type": "repeat"}}}"#,
            r#"{"gui": {"scaling": {"type": "tile", "width": 16}}}"#,
            r#"{"gui": {"scaling": {"type": "tile", "width": 0, "height": 8}}}"#,
            r#"{"gui": {"scaling": {"type": "nine_slice", "width": 100, "height": 100, "border": 0}}}"#,
            r#"{"gui": {"scaling": {"type": "nine_slice", "width": 100, "height": 100, "border": 50}}}"#,
            r#"{"gui": {"scaling": {"type": "nine_slice", "width": 100, "height": 100, "border": -1}}}"#,
            r#"{"gui": {"scaling": {"type": "nine_slice", "width": 100, "height": 100,
                "border": {"left": 1, "top": 1, "right": 1}}}}"#,
            r#"{"gui": {"scaling": {"type": "nine_slice", "width": 100, "height": 100,
                "border": {"left": -1, "top": 1, "right": 1, "bottom": 1}}}}"#,
        ] {
            assert!(Scaling::read(Some(json.as_bytes())).is_err(), "{json}");
        }
    }

    type Quad = ([i32; 4], [i32; 4]);

    fn sorted_quads(sprite: GuiSprite, target: IRect) -> Vec<Quad> {
        let corners = |r: IRect| [r.min.x, r.min.y, r.max.x, r.max.y];
        let mut out = Vec::new();
        sprite.draw(target, &mut out);
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
}
