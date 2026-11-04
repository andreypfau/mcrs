use serde::{Deserialize, Serialize};

use super::spec::AttributeValue;

/// The operation a modifier names to compose its argument onto the layer beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Override,
    AlphaBlend,
    Add,
    Subtract,
    Multiply,
    BlendToGray,
    Minimum,
    Maximum,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Append,
    Overlay,
}

#[derive(Debug, thiserror::Error)]
pub enum ModifierError {
    #[error("the {0:?} modifier is not supported")]
    Unsupported(Operation),
}

/// Composes `argument` onto `subject` as the game's modifier of the same name does.
///
/// Colours follow `ARGB`'s float forms: `add` and `subtract` clamp each colour channel and keep
/// the subject's alpha, `multiply` multiplies every channel, `alpha_blend` composites the argument
/// over the subject by the argument's alpha. An rgb colour is stored opaque, so the four-channel
/// form gives what `ARGB`'s three-channel form gives. Floats and integers follow `FloatModifier`
/// and `IntegerModifier` (integers wrap as Java's do), booleans `BooleanModifier`.
///
/// `blend_to_gray`, `append` and `overlay` are not composed yet: no shipped timeline or dimension
/// uses the first, and the other two only touch gameplay lists.
pub fn apply(
    op: Operation,
    subject: &AttributeValue,
    argument: &AttributeValue,
) -> Result<AttributeValue, ModifierError> {
    use AttributeValue as V;
    use Operation as Op;

    Ok(match (op, subject, argument) {
        (Op::Override, _, argument) => argument.clone(),

        (Op::Add, V::Float(s), V::Float(a)) => V::Float(s + a),
        (Op::Subtract, V::Float(s), V::Float(a)) => V::Float(s - a),
        (Op::Multiply, V::Float(s), V::Float(a)) => V::Float(s * a),
        (Op::Minimum, V::Float(s), V::Float(a)) => V::Float(java_min(*s, *a)),
        (Op::Maximum, V::Float(s), V::Float(a)) => V::Float(java_max(*s, *a)),
        // `Mth.lerp(alpha, subject, value)`.
        (Op::AlphaBlend, V::Float(s), V::FloatWithAlpha { value, alpha }) => {
            V::Float(s + alpha * (value - s))
        }

        (Op::Add, V::Integer(s), V::Integer(a)) => V::Integer(s.wrapping_add(*a)),
        (Op::Subtract, V::Integer(s), V::Integer(a)) => V::Integer(s.wrapping_sub(*a)),
        (Op::Multiply, V::Integer(s), V::Integer(a)) => V::Integer(s.wrapping_mul(*a)),
        (Op::Minimum, V::Integer(s), V::Integer(a)) => V::Integer(*s.min(a)),
        (Op::Maximum, V::Integer(s), V::Integer(a)) => V::Integer(*s.max(a)),

        (Op::And, V::Bool(s), V::Bool(a)) => V::Bool(*s && *a),
        (Op::Nand, V::Bool(s), V::Bool(a)) => V::Bool(!(*s && *a)),
        (Op::Or, V::Bool(s), V::Bool(a)) => V::Bool(*s || *a),
        (Op::Nor, V::Bool(s), V::Bool(a)) => V::Bool(!(*s || *a)),
        (Op::Xor, V::Bool(s), V::Bool(a)) => V::Bool(s ^ a),
        (Op::Xnor, V::Bool(s), V::Bool(a)) => V::Bool(!(s ^ a)),

        (Op::Add, V::Color(s), V::Color(a)) => V::Color(rgb_with(*s, *a, |s, a| (s + a).min(1.0))),
        (Op::Subtract, V::Color(s), V::Color(a)) => {
            V::Color(rgb_with(*s, *a, |s, a| (s - a).max(0.0)))
        }
        (Op::Multiply, V::Color(s), V::Color(a)) => V::Color(multiply(*s, *a)),
        (Op::AlphaBlend, V::Color(s), V::Color(a)) => V::Color(alpha_blend(*s, *a)),

        _ => return Err(ModifierError::Unsupported(op)),
    })
}

/// `Math.min`: a NaN on either side is the result, where `f32::min` would drop it, and of two
/// zeros the negative one, which `f32::min` leaves unspecified.
fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == b {
        if a.is_sign_negative() { a } else { b }
    } else {
        a.min(b)
    }
}

/// `Math.max`: as `java_min`, and of two zeros the positive one.
fn java_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == b {
        if a.is_sign_positive() { a } else { b }
    } else {
        a.max(b)
    }
}

/// The channel at `shift` of a packed `0xAARRGGBB`, as a float in `[0, 1]`.
fn channel(color: u32, shift: u32) -> f32 {
    ((color >> shift) & 0xFF) as f32 / 255.0
}

/// Back to eight bits. Rounded, not floored: a value that went through a float and back
/// unchanged, such as a channel multiplied by one, keeps its byte.
fn byte(value: f32, shift: u32) -> u32 {
    ((value.clamp(0.0, 1.0) * 255.0).round() as u32) << shift
}

/// `ARGB.addRgb`/`subtractRgb`: `f` on red, green and blue; the subject's alpha stays.
fn rgb_with(subject: u32, argument: u32, f: impl Fn(f32, f32) -> f32) -> u32 {
    (subject & 0xFF00_0000)
        | [16, 8, 0]
            .into_iter()
            .map(|shift| byte(f(channel(subject, shift), channel(argument, shift)), shift))
            .fold(0, |packed, c| packed | c)
}

/// `ARGB.multiply`: every channel, alpha too.
fn multiply(subject: u32, argument: u32) -> u32 {
    [24, 16, 8, 0]
        .into_iter()
        .map(|shift| byte(channel(subject, shift) * channel(argument, shift), shift))
        .fold(0, |packed, c| packed | c)
}

/// `ARGB.alphaBlend`: the argument composited over the subject. An opaque argument replaces
/// the subject and a transparent one leaves it.
fn alpha_blend(subject: u32, argument: u32) -> u32 {
    let (subject_alpha, argument_alpha) = (channel(subject, 24), channel(argument, 24));
    if argument_alpha == 1.0 {
        return argument;
    }
    if argument_alpha == 0.0 {
        return subject;
    }
    let alpha = argument_alpha + subject_alpha * (1.0 - argument_alpha);
    // `alphaBlendChannel(alpha, argumentAlpha, subject, argument)`.
    let blend = |shift| {
        let (s, a) = (channel(subject, shift), channel(argument, shift));
        byte(
            (a * argument_alpha + s * (alpha - argument_alpha)) / alpha,
            shift,
        )
    };
    byte(alpha, 24) | blend(16) | blend(8) | blend(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use AttributeValue as V;

    fn color(op: Operation, subject: u32, argument: u32) -> u32 {
        match apply(op, &V::Color(subject), &V::Color(argument)).unwrap() {
            V::Color(c) => c,
            other => panic!("not a colour: {other:?}"),
        }
    }

    #[test]
    fn the_day_timeline_darkens_the_overworld_sky_at_night() {
        // timeline/day.json multiplies the overworld's #78a7ff sky by white at noon and black at night.
        assert_eq!(
            color(Operation::Multiply, 0xFF78_A7FF, 0xFFFF_FFFF),
            0xFF78_A7FF
        );
        assert_eq!(
            color(Operation::Multiply, 0xFF78_A7FF, 0xFF00_0000),
            0xFF00_0000
        );
        // the cloud colour is argb; an opaque argument keeps its alpha...
        assert_eq!(
            color(Operation::Multiply, 0xCCFF_FFFF, 0xFF19_1926),
            0xCC19_1926
        );
        // ...and a translucent one multiplies it: 0.8 * 0.502 of 255 is 0x66.
        assert_eq!(
            color(Operation::Multiply, 0xCCFF_FFFF, 0x8019_1926),
            0x6619_1926
        );
    }

    #[test]
    fn add_and_subtract_clamp_the_colour_and_keep_the_subject_alpha() {
        assert_eq!(color(Operation::Add, 0x80F0_1000, 0xFF20_2020), 0x80FF_3020);
        assert_eq!(
            color(Operation::Subtract, 0x80F0_1000, 0xFF20_2020),
            0x80D0_0000
        );
    }

    #[test]
    fn alpha_blend_composites_by_the_argument_alpha() {
        assert_eq!(
            color(Operation::AlphaBlend, 0xFF00_0000, 0xFFFF_FFFF),
            0xFFFF_FFFF
        );
        assert_eq!(
            color(Operation::AlphaBlend, 0xFF12_3456, 0x00FF_FFFF),
            0xFF12_3456
        );
        // half white over half black: alpha 0.502 + 0.502 * 0.498 = 0.752 (0xC0), and white
        // weighs 0.502 / 0.752 of each channel (0xAA).
        assert_eq!(
            color(Operation::AlphaBlend, 0x8000_0000, 0x80FF_FFFF),
            0xC0AA_AAAA
        );
        // half white over opaque black: opaque mid grey.
        assert_eq!(
            color(Operation::AlphaBlend, 0xFF00_0000, 0x80FF_FFFF),
            0xFF80_8080
        );
    }

    #[test]
    fn floats_integers_and_booleans() {
        let f = |op, s: f32, a: f32| apply(op, &V::Float(s), &V::Float(a)).unwrap();
        assert_eq!(f(Operation::Multiply, 0.5, 0.24), V::Float(0.12));
        assert_eq!(f(Operation::Maximum, 0.2, 0.7), V::Float(0.7));
        // Java picks the zero by sign; `f32::min`/`max` do not promise which.
        let sign = |op, s, a| match f(op, s, a) {
            V::Float(x) => x.is_sign_negative(),
            other => panic!("not a float: {other:?}"),
        };
        assert!(sign(Operation::Minimum, 0.0, -0.0) && sign(Operation::Minimum, -0.0, 0.0));
        assert!(!sign(Operation::Maximum, 0.0, -0.0) && !sign(Operation::Maximum, -0.0, 0.0));
        assert_eq!(f(Operation::Subtract, 1.0, 0.25), V::Float(0.75));
        assert_eq!(
            apply(
                Operation::AlphaBlend,
                &V::Float(0.0),
                &V::FloatWithAlpha {
                    value: 1.0,
                    alpha: 0.25
                }
            )
            .unwrap(),
            V::Float(0.25)
        );
        assert_eq!(
            apply(Operation::Add, &V::Integer(i32::MAX), &V::Integer(1)).unwrap(),
            V::Integer(i32::MIN)
        );
        assert_eq!(
            apply(Operation::Xnor, &V::Bool(false), &V::Bool(false)).unwrap(),
            V::Bool(true)
        );
        assert_eq!(
            apply(Operation::Nand, &V::Bool(true), &V::Bool(true)).unwrap(),
            V::Bool(false)
        );
    }

    #[test]
    fn what_is_not_composed_says_so() {
        let gray = V::BlendToGray {
            brightness: 0.5,
            factor: 0.5,
        };
        assert!(matches!(
            apply(Operation::BlendToGray, &V::Color(0xFF00_0000), &gray),
            Err(ModifierError::Unsupported(Operation::BlendToGray))
        ));
    }
}
