use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VillagerType {
    #[serde(rename = "minecraft:desert")]
    Desert,
    #[serde(rename = "minecraft:jungle")]
    Jungle,
    #[default]
    #[serde(rename = "minecraft:plains")]
    Plains,
    #[serde(rename = "minecraft:savanna")]
    Savanna,
    #[serde(rename = "minecraft:snow")]
    Snow,
    #[serde(rename = "minecraft:swamp")]
    Swamp,
    #[serde(rename = "minecraft:taiga")]
    Taiga,
}

impl VillagerType {
    pub const ALL: [Self; 7] = [
        Self::Desert,
        Self::Jungle,
        Self::Plains,
        Self::Savanna,
        Self::Snow,
        Self::Swamp,
        Self::Taiga,
    ];

    pub const fn protocol_id(self) -> u16 {
        self as u16
    }
}
