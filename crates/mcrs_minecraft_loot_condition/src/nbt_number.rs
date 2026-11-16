/// A number tag that a command storage holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NbtNumber {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl NbtNumber {
    /// The value for an int provider (LOOT-NUM-017): a long gives its low 32 bits, and a float
    /// or a double gives `toI32` of its value.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low 32 bits of a long, and toI32 saturates"
    )]
    pub fn to_i32(self) -> i32 {
        match self {
            NbtNumber::Byte(value) => i32::from(value),
            NbtNumber::Short(value) => i32::from(value),
            NbtNumber::Int(value) => value,
            NbtNumber::Long(value) => value as i32,
            NbtNumber::Float(value) => value as i32,
            NbtNumber::Double(value) => value as i32,
        }
    }

    /// The value for a float provider (LOOT-NUM-017): an integer gives the nearest f32, and a
    /// double gives its narrow.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the nearest f32 of the number"
    )]
    pub fn to_f32(self) -> f32 {
        match self {
            NbtNumber::Byte(value) => f32::from(value),
            NbtNumber::Short(value) => f32::from(value),
            NbtNumber::Int(value) => value as f32,
            NbtNumber::Long(value) => value as f32,
            NbtNumber::Float(value) => value,
            NbtNumber::Double(value) => value as f32,
        }
    }
}
