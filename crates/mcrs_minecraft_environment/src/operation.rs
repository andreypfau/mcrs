use std::fmt;
use std::str::FromStr;

use mcrs_minecraft_nbt::tag::NbtTag;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::attribute_value::{AttributeValue, Interpolation, channels, integer, lerp, read_float};
use crate::kind::AttributeType;
use crate::mob_spawns::MobSpawnSettings;

/// A modifier: how an argument changes the value of the layers below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    Override,
    Add,
    Subtract,
    Multiply,
    Minimum,
    Maximum,
    AlphaBlend,
    BlendToGray,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Append,
    Overlay,
}

impl Operation {
    const ALL: [Operation; 16] = [
        Operation::Override,
        Operation::Add,
        Operation::Subtract,
        Operation::Multiply,
        Operation::Minimum,
        Operation::Maximum,
        Operation::AlphaBlend,
        Operation::BlendToGray,
        Operation::And,
        Operation::Nand,
        Operation::Or,
        Operation::Nor,
        Operation::Xor,
        Operation::Xnor,
        Operation::Append,
        Operation::Overlay,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Operation::Override => "override",
            Operation::Add => "add",
            Operation::Subtract => "subtract",
            Operation::Multiply => "multiply",
            Operation::Minimum => "minimum",
            Operation::Maximum => "maximum",
            Operation::AlphaBlend => "alpha_blend",
            Operation::BlendToGray => "blend_to_gray",
            Operation::And => "and",
            Operation::Nand => "nand",
            Operation::Or => "or",
            Operation::Nor => "nor",
            Operation::Xor => "xor",
            Operation::Xnor => "xnor",
            Operation::Append => "append",
            Operation::Overlay => "overlay",
        }
    }

    /// The kind of argument the modifier takes on an attribute of `kind`, or `None` when the
    /// type does not accept the modifier.
    #[must_use]
    pub fn argument_kind(self, kind: AttributeType) -> Option<ArgumentKind> {
        use AttributeType as T;
        use Operation::*;
        Some(match (kind, self) {
            (_, Override) => ArgumentKind::Value(kind),
            (T::Float | T::AngleDegrees, Add | Subtract | Multiply | Minimum | Maximum) => {
                ArgumentKind::Value(T::Float)
            }
            (T::Float | T::AngleDegrees, AlphaBlend) => ArgumentKind::FloatBlend,
            (T::Integer, Add | Subtract | Multiply | Minimum | Maximum) => {
                ArgumentKind::Value(T::Integer)
            }
            (T::Boolean, And | Nand | Or | Nor | Xor | Xnor) => ArgumentKind::Value(T::Boolean),
            (T::RgbColor, Add | Subtract | Multiply) | (T::ArgbColor, Add | Subtract) => {
                ArgumentKind::Value(T::RgbColor)
            }
            (T::RgbColor, AlphaBlend) | (T::ArgbColor, Multiply | AlphaBlend) => {
                ArgumentKind::Value(T::ArgbColor)
            }
            (T::RgbColor | T::ArgbColor, BlendToGray) => ArgumentKind::Gray,
            (T::AmbientParticles, Append) => ArgumentKind::Value(kind),
            (T::MobSpawnSettings, Overlay) => ArgumentKind::Value(kind),
            _ => return None,
        })
    }

    /// The modifier applied to `subject`, or `None` when the argument does not fit it.
    #[must_use]
    pub fn apply(self, subject: &AttributeValue, argument: &Argument) -> Option<AttributeValue> {
        use AttributeValue as V;
        use Operation::*;
        Some(match (self, subject, argument) {
            (Override, _, Argument::Value(value)) => value.clone(),
            (_, V::Float(s), Argument::Value(V::Float(a))) => V::Float(float(self, *s, *a)?),
            (AlphaBlend, V::Float(s), Argument::FloatBlend { value, alpha }) => {
                V::Float(lerp(*alpha, *s, *value))
            }
            (_, V::Integer(s), Argument::Value(V::Integer(a))) => V::Integer(int(self, *s, *a)?),
            (_, V::Boolean(s), Argument::Value(V::Boolean(a))) => {
                V::Boolean(boolean(self, *s, *a)?)
            }
            (_, V::Rgb(s), Argument::Value(V::Rgb(a))) => V::Rgb(rgb_channels(self, *s, *a)?),
            (_, V::Argb(s), Argument::Value(V::Rgb(a))) => V::Argb(argb_with_rgb(self, *s, *a)?),
            (AlphaBlend, V::Rgb(s), Argument::Value(V::Argb(a))) => V::Rgb(rgb_alpha_blend(*s, *a)),
            (Multiply, V::Argb(s), Argument::Value(V::Argb(a))) => {
                V::Argb([s[0] * a[0], s[1] * a[1], s[2] * a[2], s[3] * a[3]])
            }
            (AlphaBlend, V::Argb(s), Argument::Value(V::Argb(a))) => {
                V::Argb(argb_alpha_blend(*s, *a))
            }
            (BlendToGray, V::Rgb(s), Argument::Gray { brightness, factor }) => {
                let [red, green, blue] = s;
                let [red, green, blue, _] =
                    blend_to_gray([*red, *green, *blue, 0.0], *brightness, *factor);
                V::Rgb([red, green, blue])
            }
            (BlendToGray, V::Argb(s), Argument::Gray { brightness, factor }) => {
                V::Argb(blend_to_gray(*s, *brightness, *factor))
            }
            (Append, V::AmbientParticles(s), Argument::Value(V::AmbientParticles(a))) => {
                V::AmbientParticles(s.iter().chain(a).cloned().collect())
            }
            (Overlay, V::MobSpawnSettings(s), Argument::Value(V::MobSpawnSettings(a))) => {
                V::MobSpawnSettings(overlay(s, a))
            }
            _ => return None,
        })
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Operation {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Operation::ALL
            .into_iter()
            .find(|operation| operation.name() == text)
            .ok_or_else(|| format!("unknown modifier `{text}`"))
    }
}

/// The shape of the argument a modifier takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentKind {
    /// A value of the given type.
    Value(AttributeType),
    /// A float `value` and an `alpha` in `[0, 1]`.
    FloatBlend,
    /// A `brightness` and a `factor`, both in `[0, 1]`.
    Gray,
}

/// The argument of one modifier.
#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    Value(AttributeValue),
    FloatBlend { value: f32, alpha: f32 },
    Gray { brightness: f32, factor: f32 },
}

fn unit(name: &str, tag: Option<&NbtTag>) -> Result<f32, String> {
    let value = read_float(tag.ok_or_else(|| format!("missing `{name}`"))?)?;
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(format!("`{name}` {value} is not in range [0; 1]"))
    }
}

impl Argument {
    /// Reads an argument of `kind`. A value is not checked against a range here.
    pub fn read(kind: ArgumentKind, tag: &NbtTag) -> Result<Argument, String> {
        match (kind, tag) {
            (ArgumentKind::Value(kind), tag) => {
                AttributeValue::read(kind, tag).map(Argument::Value)
            }
            (ArgumentKind::FloatBlend, NbtTag::Compound(fields)) => {
                let value = read_float(fields.get("value").ok_or("missing `value`")?)?;
                let alpha = match fields.get("alpha") {
                    Some(alpha) => unit("alpha", Some(alpha))?,
                    None => 1.0,
                };
                Ok(Argument::FloatBlend { value, alpha })
            }
            (ArgumentKind::FloatBlend, tag) => Ok(Argument::FloatBlend {
                value: read_float(tag)?,
                alpha: 1.0,
            }),
            (ArgumentKind::Gray, NbtTag::Compound(fields)) => Ok(Argument::Gray {
                brightness: unit("brightness", fields.get("brightness"))?,
                factor: unit("factor", fields.get("factor"))?,
            }),
            (ArgumentKind::Gray, _) => Err("expected `brightness` and `factor`".to_owned()),
        }
    }

    /// The interpolation of a track's arguments between two keyframes.
    #[must_use]
    pub fn interpolate(
        operation: Operation,
        kind: AttributeType,
        t: f32,
        from: &Argument,
        to: &Argument,
    ) -> Argument {
        match (from, to) {
            (
                Argument::FloatBlend { value, alpha },
                Argument::FloatBlend {
                    value: to_value,
                    alpha: to_alpha,
                },
            ) => Argument::FloatBlend {
                value: lerp(t, *value, *to_value),
                alpha: lerp(t, *alpha, *to_alpha),
            },
            (
                Argument::Gray { brightness, factor },
                Argument::Gray {
                    brightness: to_brightness,
                    factor: to_factor,
                },
            ) => Argument::Gray {
                brightness: lerp(t, *brightness, *to_brightness),
                factor: lerp(t, *factor, *to_factor),
            },
            (Argument::Value(a), Argument::Value(b)) => {
                Argument::Value(interpolate_value(operation, kind, t, a, b))
            }
            _ if t >= 1.0 => to.clone(),
            _ => from.clone(),
        }
    }
}

fn interpolate_value(
    operation: Operation,
    kind: AttributeType,
    t: f32,
    a: &AttributeValue,
    b: &AttributeValue,
) -> AttributeValue {
    use AttributeValue as V;
    match (operation, a, b) {
        (Operation::Override | Operation::Append, _, _) => {
            AttributeValue::interpolate(kind, Interpolation::Keyframe, t, a, b)
        }
        (_, V::Float(a), V::Float(b)) => V::Float(lerp(t, *a, *b)),
        (_, V::Integer(a), V::Integer(b)) => V::Integer(integer(t, *a, *b)),
        (_, V::Rgb(a), V::Rgb(b)) => V::Rgb(channels(t, *a, *b)),
        (_, V::Argb(a), V::Argb(b)) => V::Argb(channels(t, *a, *b)),
        _ if t >= 1.0 => b.clone(),
        _ => a.clone(),
    }
}

impl Serialize for Argument {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Argument::Value(value) => value.serialize(serializer),
            Argument::FloatBlend { value, alpha } if *alpha == 1.0 => {
                serializer.serialize_f32(*value)
            }
            Argument::FloatBlend { value, alpha } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("value", value)?;
                map.serialize_entry("alpha", alpha)?;
                map.end()
            }
            Argument::Gray { brightness, factor } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("brightness", brightness)?;
                map.serialize_entry("factor", factor)?;
                map.end()
            }
        }
    }
}

fn float(operation: Operation, s: f32, a: f32) -> Option<f32> {
    Some(match operation {
        Operation::Add => s + a,
        Operation::Subtract => s - a,
        Operation::Multiply => s * a,
        Operation::Minimum => minimum(s, a),
        Operation::Maximum => maximum(s, a),
        _ => return None,
    })
}

fn minimum(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a < b {
        a
    } else if b < a {
        b
    } else if a.is_sign_negative() {
        a
    } else {
        b
    }
}

fn maximum(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a > b {
        a
    } else if b > a {
        b
    } else if a.is_sign_positive() {
        a
    } else {
        b
    }
}

fn int(operation: Operation, s: i32, a: i32) -> Option<i32> {
    Some(match operation {
        Operation::Add => s.wrapping_add(a),
        Operation::Subtract => s.wrapping_sub(a),
        Operation::Multiply => s.wrapping_mul(a),
        Operation::Minimum => s.min(a),
        Operation::Maximum => s.max(a),
        _ => return None,
    })
}

fn boolean(operation: Operation, s: bool, a: bool) -> Option<bool> {
    Some(match operation {
        Operation::And => s && a,
        Operation::Nand => !(s && a),
        Operation::Or => s || a,
        Operation::Nor => !(s || a),
        Operation::Xor => s != a,
        Operation::Xnor => s == a,
        _ => return None,
    })
}

fn rgb_channels(operation: Operation, s: [f32; 3], a: [f32; 3]) -> Option<[f32; 3]> {
    let mut out = s;
    for (slot, a) in out.iter_mut().zip(a) {
        *slot = match operation {
            Operation::Add => (*slot + a).min(1.0),
            Operation::Subtract => (*slot - a).max(0.0),
            Operation::Multiply => *slot * a,
            _ => return None,
        };
    }
    Some(out)
}

fn argb_with_rgb(operation: Operation, s: [f32; 4], a: [f32; 3]) -> Option<[f32; 4]> {
    if !matches!(operation, Operation::Add | Operation::Subtract) {
        return None;
    }
    let [red, green, blue] = rgb_channels(operation, [s[0], s[1], s[2]], a)?;
    Some([red, green, blue, s[3]])
}

fn rgb_alpha_blend(s: [f32; 3], a: [f32; 4]) -> [f32; 3] {
    let alpha = a[3];
    if alpha == 0.0 {
        return s;
    }
    channels(alpha, s, [a[0], a[1], a[2]])
}

fn argb_alpha_blend(s: [f32; 4], a: [f32; 4]) -> [f32; 4] {
    let alpha = a[3];
    if alpha == 1.0 {
        return a;
    }
    if alpha == 0.0 {
        return s;
    }
    let combined = alpha + s[3] * (1.0 - alpha);
    let blend = |a_c: f32, s_c: f32| (a_c * alpha + s_c * (combined - alpha)) / combined;
    [
        blend(a[0], s[0]),
        blend(a[1], s[1]),
        blend(a[2], s[2]),
        combined,
    ]
}

fn blend_to_gray(s: [f32; 4], brightness: f32, factor: f32) -> [f32; 4] {
    let gray = 0.3 * s[0] + 0.59 * s[1] + 0.11 * s[2];
    let target = (gray * brightness).clamp(0.0, 1.0);
    [
        lerp(factor, s[0], target),
        lerp(factor, s[1], target),
        lerp(factor, s[2], target),
        s[3],
    ]
}

fn overlay(subject: &MobSpawnSettings, argument: &MobSpawnSettings) -> MobSpawnSettings {
    let mut out = subject.clone();
    for (category, spawns) in &argument.spawns_by_category {
        out.spawns_by_category.insert(*category, spawns.clone());
    }
    for (entity, cost) in &argument.spawn_costs {
        out.spawn_costs.insert(entity.clone(), cost.clone());
    }
    out
}
