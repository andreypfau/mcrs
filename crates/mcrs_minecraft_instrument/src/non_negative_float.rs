use serde::{Deserialize, Serialize};

/// A finite float that is 0 or more. The value `-0.0` is refused.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32")]
pub struct NonNegativeFloat(pub f32);

impl TryFrom<f32> for NonNegativeFloat {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        if !(value.is_sign_positive() && value <= f32::MAX) {
            return Err(format!("Value must be non-negative: {value}"));
        }
        Ok(Self(value))
    }
}
