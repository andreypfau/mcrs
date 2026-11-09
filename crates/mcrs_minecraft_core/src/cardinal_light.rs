use crate::Direction;
use serde::{Deserialize, Serialize};

/// The directional shade factors of a dimension type, one table per value of its
/// `cardinal_light`.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardinalLight {
    #[default]
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "nether")]
    Nether,
}

impl CardinalLight {
    #[must_use]
    pub const fn factor(self, direction: Direction) -> f32 {
        match (self, direction) {
            (CardinalLight::Default, Direction::Down) => 0.5,
            (CardinalLight::Default, Direction::Up) => 1.0,
            (CardinalLight::Nether, Direction::Down | Direction::Up) => 0.9,
            (_, Direction::North | Direction::South) => 0.8,
            (_, Direction::West | Direction::East) => 0.6,
        }
    }
}
