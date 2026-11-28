use mcrs_minecraft_nbt::tag::NbtTag;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Whole(i64),
    Fraction(f64),
}

impl Number {
    #[must_use]
    pub fn of(tag: &NbtTag) -> Option<Number> {
        match tag {
            NbtTag::Byte(value) => Some(Number::Whole(i64::from(*value))),
            NbtTag::Short(value) => Some(Number::Whole(i64::from(*value))),
            NbtTag::Int(value) => Some(Number::Whole(i64::from(*value))),
            NbtTag::Long(value) => Some(Number::Whole(*value)),
            NbtTag::Float(value) => Some(Number::Fraction(f64::from(*value))),
            NbtTag::Double(value) => Some(Number::Fraction(*value)),
            _ => None,
        }
    }

    #[must_use]
    pub fn to_f64(self) -> f64 {
        match self {
            Number::Whole(value) => value as f64,
            Number::Fraction(value) => value,
        }
    }

    pub(crate) fn to_i64(self) -> i64 {
        match self {
            Number::Whole(value) => value,
            Number::Fraction(value) => floor(value),
        }
    }

    pub(crate) fn to_i32(self) -> i32 {
        match self {
            Number::Whole(value) => value as i32,
            Number::Fraction(value) => floor(value).clamp(i32::MIN.into(), i32::MAX.into()) as i32,
        }
    }

    pub(crate) fn to_i8(self) -> i8 {
        match self {
            Number::Whole(value) => value as i8,
            Number::Fraction(value) => floor(value).clamp(i8::MIN.into(), i8::MAX.into()) as i8,
        }
    }
}

fn floor(value: f64) -> i64 {
    let truncated = value as i64;
    if truncated as f64 > value {
        truncated.saturating_sub(1)
    } else {
        truncated
    }
}
