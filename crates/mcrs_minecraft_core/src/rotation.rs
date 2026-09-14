use serde::{Deserialize, Serialize};

use crate::Direction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rotation {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "clockwise_90")]
    Clockwise90,
    #[serde(rename = "180")]
    Clockwise180,
    #[serde(rename = "counterclockwise_90")]
    Counterclockwise90,
}

impl Rotation {
    /// `Rotation.values()`, the order `nextInt(4)` indexes.
    pub const ALL: [Rotation; 4] = [
        Rotation::None,
        Rotation::Clockwise90,
        Rotation::Clockwise180,
        Rotation::Counterclockwise90,
    ];

    pub fn rotate(self, direction: Direction) -> Direction {
        if direction.is_vertical() {
            return direction;
        }
        match self {
            Rotation::None => direction,
            Rotation::Clockwise90 => direction.clockwise(),
            Rotation::Clockwise180 => direction.opposite(),
            Rotation::Counterclockwise90 => direction.opposite().clockwise(),
        }
    }
}
