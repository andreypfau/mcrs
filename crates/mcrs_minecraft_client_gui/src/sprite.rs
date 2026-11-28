use std::fmt;

use bevy::math::{IRect, IVec2, Rect};
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuiSprite {
    pub(crate) texels: Rect,
    pub(crate) scaling: Scaling,
}

impl GuiSprite {
    pub fn new(texels: IRect, scaling: Scaling) -> Self {
        Self {
            texels: texels.as_rect(),
            scaling,
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
}
