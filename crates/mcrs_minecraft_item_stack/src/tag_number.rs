use mcrs_minecraft_nbt::tag::NbtTag;

/// A number of any numeric tag kind, which the saved reader converts to the kind of its field.
#[derive(Clone, Copy)]
pub(crate) enum TagNumber {
    Integer(i64),
    Float(f32),
    Double(f64),
}

impl TagNumber {
    pub(crate) fn of(tag: &NbtTag) -> Option<Self> {
        match tag {
            NbtTag::Byte(value) => Some(Self::Integer(i64::from(*value))),
            NbtTag::Short(value) => Some(Self::Integer(i64::from(*value))),
            NbtTag::Int(value) => Some(Self::Integer(i64::from(*value))),
            NbtTag::Long(value) => Some(Self::Integer(*value)),
            NbtTag::Float(value) => Some(Self::Float(*value)),
            NbtTag::Double(value) => Some(Self::Double(*value)),
            _ => None,
        }
    }

    // `as` is the conversion of the saved form: the low 32 bits of an integer; for a float, the
    // value toward zero, the nearest limit out of range, and 0 for a NaN.
    pub(crate) fn to_int(self) -> i32 {
        match self {
            Self::Integer(value) => value as i32,
            Self::Float(value) => value as i32,
            Self::Double(value) => value as i32,
        }
    }

    pub(crate) fn to_float(self) -> f32 {
        match self {
            Self::Integer(value) => value as f32,
            Self::Float(value) => value,
            Self::Double(value) => value as f32,
        }
    }

    pub(crate) fn to_double(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => f64::from(value),
            Self::Double(value) => value,
        }
    }

    pub(crate) fn is_true(self) -> bool {
        match self {
            Self::Integer(value) => value != 0,
            Self::Float(value) => value != 0.0,
            Self::Double(value) => value != 0.0,
        }
    }
}
