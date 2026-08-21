//! `LerpFunction`: how two values of an attribute are blended.
//!
//! An [`AttributeType`] carries four of them, one per blend the game performs:
//! between the keyframes of a track, across a layer fading in or out, across
//! neighbouring biomes, and between two ticks when rendering. `ofInterpolated`
//! gives the first three the same function and only lets the fourth differ,
//! which is why they are derived here from one `interpolated_lerp` rather than
//! stored four times.

use serde_json::Value;

use super::modifier::Operation;
use super::registry::{AttributeSpec, AttributeType, AttributeValue};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lerp {
    /// `Mth.lerp`.
    Float,
    /// `Mth.lerpInt`.
    Integer,
    /// `ARGB.srgbLerp`: per-channel integer lerp on the sRGB bytes. The
    /// reference also has `linearLerp`, which goes through a linear-space
    /// table; attributes do not use it.
    SrgbColor,
    /// `LerpFunction.ofDegrees`: the shortest arc, snapping straight to `to`
    /// once the gap reaches `max_delta`.
    Degrees { max_delta: f32 },
    /// `LerpFunction.ofStep`: `from` holds until alpha reaches `threshold`.
    Step { threshold: f32 },
    /// `Mth.lerp` on both fields of a `FloatWithAlpha` argument.
    FloatWithAlpha,
    /// `Mth.lerp` on both fields of a `ColorModifier.BlendToGray` argument.
    BlendToGray,
    /// `LerpFunction.ofListCrossFade`: both lists are emitted, with the
    /// probabilities of one scaled by `1 - alpha` and of the other by `alpha`.
    ListCrossFade,
}

impl Lerp {
    pub fn apply(self, alpha: f32, from: &AttributeValue, to: &AttributeValue) -> AttributeValue {
        use AttributeValue as V;

        match (self, from, to) {
            (Self::Float, V::Float(a), V::Float(b)) => V::Float(lerp(alpha, *a, *b)),
            (Self::Integer, V::Integer(a), V::Integer(b)) => V::Integer(lerp_int(alpha, *a, *b)),
            (Self::SrgbColor, V::Color(a), V::Color(b)) => V::Color(srgb_lerp(alpha, *a, *b)),
            (Self::Degrees { max_delta }, V::Float(a), V::Float(b)) => {
                let delta = wrap_degrees(b - a);
                V::Float(if delta.abs() >= max_delta {
                    *b
                } else {
                    a + alpha * delta
                })
            }
            (
                Self::FloatWithAlpha,
                V::FloatWithAlpha {
                    value: v0,
                    alpha: a0,
                },
                V::FloatWithAlpha {
                    value: v1,
                    alpha: a1,
                },
            ) => V::FloatWithAlpha {
                value: lerp(alpha, *v0, *v1),
                alpha: lerp(alpha, *a0, *a1),
            },
            (
                Self::BlendToGray,
                V::BlendToGray {
                    brightness: b0,
                    factor: f0,
                },
                V::BlendToGray {
                    brightness: b1,
                    factor: f1,
                },
            ) => V::BlendToGray {
                brightness: lerp(alpha, *b0, *b1),
                factor: lerp(alpha, *f0, *f1),
            },
            (Self::ListCrossFade, V::List(a), V::List(b)) => V::List(cross_fade(alpha, a, b)),
            // Both endpoints of a segment come from the same attribute and the
            // same operation, so a mismatched pair cannot be built from parsed
            // data; stepping is what the non-interpolated types do anyway.
            (_, from, to) => {
                let threshold = match self {
                    Self::Step { threshold } => threshold,
                    _ => 1.0,
                };
                if alpha >= threshold {
                    to.clone()
                } else {
                    from.clone()
                }
            }
        }
    }
}

impl AttributeType {
    /// The one function `ofInterpolated` shares between keyframes, state
    /// changes and biomes. `None` for the types built by `ofNotInterpolated`,
    /// which step at a different threshold for each of the four blends.
    fn interpolated_lerp(self) -> Option<Lerp> {
        use AttributeType as T;

        Some(match self {
            T::Float | T::AngleDegrees => Lerp::Float,
            T::RgbColor | T::ArgbColor => Lerp::SrgbColor,
            T::Integer => Lerp::Integer,
            T::AmbientParticles => Lerp::ListCrossFade,
            _ => return None,
        })
    }

    pub fn keyframe_lerp(self) -> Lerp {
        self.interpolated_lerp()
            .unwrap_or(Lerp::Step { threshold: 1.0 })
    }

    pub fn state_change_lerp(self) -> Lerp {
        self.interpolated_lerp()
            .unwrap_or(Lerp::Step { threshold: 0.0 })
    }

    pub fn spatial_lerp(self) -> Lerp {
        self.interpolated_lerp()
            .unwrap_or(Lerp::Step { threshold: 0.5 })
    }

    /// The only one `ofInterpolated` lets differ: `ANGLE_DEGREES` smooths
    /// between two ticks along the shortest arc. A keyframe track that runs
    /// 0 → 360 across its period would collapse to a constant under it, which
    /// is why [`AttributeType::keyframe_lerp`] is a plain float lerp instead.
    pub fn partial_tick_lerp(self) -> Lerp {
        match self {
            Self::AngleDegrees => Lerp::Degrees { max_delta: 90.0 },
            other => other
                .interpolated_lerp()
                .unwrap_or(Lerp::Step { threshold: 0.0 }),
        }
    }
}

impl AttributeSpec {
    /// `AttributeModifier.argumentKeyframeLerp`: a keyframe carries the
    /// modifier's argument rather than the attribute's value, so the lerp
    /// follows the argument's shape — the same `(type, operation)` dispatch
    /// [`AttributeSpec::parse_argument`] uses to parse it.
    pub fn argument_keyframe_lerp(&self, op: Operation) -> Lerp {
        use AttributeType as T;
        use Operation::*;

        match (self.ty, op) {
            (T::Float | T::AngleDegrees, AlphaBlend) => Lerp::FloatWithAlpha,
            (T::Float | T::AngleDegrees, Add | Subtract | Multiply | Minimum | Maximum) => {
                Lerp::Float
            }
            (T::Integer, Add | Subtract | Multiply | Minimum | Maximum) => Lerp::Integer,
            (T::RgbColor | T::ArgbColor, BlendToGray) => Lerp::BlendToGray,
            (T::RgbColor | T::ArgbColor, AlphaBlend | Add | Subtract | Multiply) => Lerp::SrgbColor,
            // `override` takes the attribute's own value, and the boolean, list
            // and mob-spawn libraries defer to the type's keyframe lerp too.
            _ => self.ty.keyframe_lerp(),
        }
    }
}

fn lerp(alpha: f32, from: f32, to: f32) -> f32 {
    from + alpha * (to - from)
}

/// `Mth.lerpInt`: the step is floored, so the result leaves `from` only once a
/// whole unit has accumulated.
fn lerp_int(alpha: f32, from: i32, to: i32) -> i32 {
    from.wrapping_add((alpha * to.wrapping_sub(from) as f32).floor() as i32)
}

fn srgb_lerp(alpha: f32, from: u32, to: u32) -> u32 {
    let channel = |shift: u32| {
        let at = |color: u32| (color >> shift & 0xFF) as i32;
        (lerp_int(alpha, at(from), at(to)) as u32 & 0xFF) << shift
    };
    channel(24) | channel(16) | channel(8) | channel(0)
}

/// `Mth.wrapDegrees`.
fn wrap_degrees(angle: f32) -> f32 {
    let mut wrapped = angle % 360.0;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

fn cross_fade(alpha: f32, from: &[Value], to: &[Value]) -> Vec<Value> {
    if alpha == 0.0 {
        return from.to_vec();
    }
    if alpha == 1.0 {
        return to.to_vec();
    }
    from.iter()
        .map(|particle| scale_probability(particle, 1.0 - alpha))
        .chain(to.iter().map(|particle| scale_probability(particle, alpha)))
        .collect()
}

fn scale_probability(particle: &Value, scale: f32) -> Value {
    let mut scaled = particle.clone();
    if let Some(field) = scaled.get_mut("probability")
        && let Some(probability) = field.as_f64()
        && let Some(number) = serde_json::Number::from_f64((probability as f32 * scale) as f64)
    {
        *field = Value::Number(number);
    }
    scaled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::attribute;
    use serde_json::json;

    #[test]
    fn an_angle_track_lerps_the_long_way_between_keyframes() {
        let sun_angle = attribute("minecraft:visual/sun_angle").unwrap();
        let keyframe = sun_angle.argument_keyframe_lerp(Operation::Override);
        assert_eq!(keyframe, Lerp::Float);
        assert_eq!(
            keyframe.apply(
                0.5,
                &AttributeValue::Float(0.0),
                &AttributeValue::Float(360.0)
            ),
            AttributeValue::Float(180.0),
            "a shortest-arc lerp would collapse a full revolution to a constant"
        );

        // …while the partial-tick lerp, which is the shortest arc, does exactly that.
        assert_eq!(
            sun_angle.ty.partial_tick_lerp(),
            Lerp::Degrees { max_delta: 90.0 }
        );
        assert_eq!(
            sun_angle.ty.partial_tick_lerp().apply(
                0.5,
                &AttributeValue::Float(0.0),
                &AttributeValue::Float(360.0)
            ),
            AttributeValue::Float(0.0)
        );
    }

    #[test]
    fn colors_lerp_per_channel_on_the_srgb_bytes() {
        let sunrise = attribute("minecraft:visual/sunrise_sunset_color").unwrap();
        let lerp = sunrise.argument_keyframe_lerp(Operation::Override);
        assert_eq!(lerp, Lerp::SrgbColor);
        assert_eq!(
            lerp.apply(
                0.5,
                &AttributeValue::Color(0x0000_0000),
                &AttributeValue::Color(0xFFFF_FFFF)
            ),
            AttributeValue::Color(0x7F7F_7F7F),
            "the step is floored, so half of 255 is 127"
        );
    }

    #[test]
    fn a_not_interpolated_type_holds_until_the_end_of_the_segment() {
        let moon_phase = attribute("minecraft:visual/moon_phase").unwrap();
        let lerp = moon_phase.argument_keyframe_lerp(Operation::Override);
        assert_eq!(lerp, Lerp::Step { threshold: 1.0 });

        let full = AttributeValue::Opaque(json!("full_moon"));
        let waning = AttributeValue::Opaque(json!("waning_gibbous"));
        assert_eq!(lerp.apply(0.999, &full, &waning), full);
        assert_eq!(lerp.apply(1.0, &full, &waning), waning);

        // the other three blends of a non-interpolated type step elsewhere
        assert_eq!(
            moon_phase.ty.state_change_lerp(),
            Lerp::Step { threshold: 0.0 }
        );
        assert_eq!(moon_phase.ty.spatial_lerp(), Lerp::Step { threshold: 0.5 });
        assert_eq!(
            moon_phase.ty.partial_tick_lerp(),
            Lerp::Step { threshold: 0.0 }
        );
    }

    #[test]
    fn a_modifier_argument_lerps_as_the_argument_not_as_the_attribute() {
        // `sky_color` is an RGB attribute, but an `alpha_blend` argument is ARGB
        // and a `blend_to_gray` argument is a pair of floats.
        let sky_color = attribute("minecraft:visual/sky_color").unwrap();
        assert_eq!(
            sky_color.argument_keyframe_lerp(Operation::Multiply),
            Lerp::SrgbColor
        );
        assert_eq!(
            sky_color.argument_keyframe_lerp(Operation::BlendToGray),
            Lerp::BlendToGray
        );

        // …and a float `alpha_blend` argument is a `{value, alpha}` pair.
        let volume = attribute("minecraft:audio/music_volume").unwrap();
        assert_eq!(
            volume.argument_keyframe_lerp(Operation::AlphaBlend),
            Lerp::FloatWithAlpha
        );
        assert_eq!(
            volume.argument_keyframe_lerp(Operation::AlphaBlend).apply(
                0.25,
                &AttributeValue::FloatWithAlpha {
                    value: 0.0,
                    alpha: 0.0
                },
                &AttributeValue::FloatWithAlpha {
                    value: 1.0,
                    alpha: 1.0
                },
            ),
            AttributeValue::FloatWithAlpha {
                value: 0.25,
                alpha: 0.25
            }
        );

        // a boolean modifier steps whatever the operation
        let creaking = attribute("minecraft:gameplay/creaking_active").unwrap();
        assert_eq!(
            creaking.argument_keyframe_lerp(Operation::Or),
            Lerp::Step { threshold: 1.0 }
        );
    }

    #[test]
    fn ambient_particles_cross_fade_by_scaling_both_probabilities() {
        let particles = attribute("minecraft:visual/ambient_particles").unwrap();
        let lerp = particles.argument_keyframe_lerp(Operation::Append);
        assert_eq!(lerp, Lerp::ListCrossFade);

        let from = AttributeValue::List(vec![
            json!({"particle": "minecraft:ash", "probability": 1.0}),
        ]);
        let to = AttributeValue::List(vec![
            json!({"particle": "minecraft:spore_blossom_air", "probability": 0.5}),
        ]);
        assert_eq!(
            lerp.apply(0.25, &from, &to),
            AttributeValue::List(vec![
                json!({"particle": "minecraft:ash", "probability": 0.75}),
                json!({"particle": "minecraft:spore_blossom_air", "probability": 0.125}),
            ])
        );
        assert_eq!(lerp.apply(0.0, &from, &to), from);
        assert_eq!(lerp.apply(1.0, &from, &to), to);
    }
}
