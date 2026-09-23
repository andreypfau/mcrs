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

    pub const fn protocol_id(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VillagerProfession {
    #[default]
    #[serde(rename = "minecraft:none")]
    None,
    #[serde(rename = "minecraft:armorer")]
    Armorer,
    #[serde(rename = "minecraft:butcher")]
    Butcher,
    #[serde(rename = "minecraft:cartographer")]
    Cartographer,
    #[serde(rename = "minecraft:cleric")]
    Cleric,
    #[serde(rename = "minecraft:farmer")]
    Farmer,
    #[serde(rename = "minecraft:fisherman")]
    Fisherman,
    #[serde(rename = "minecraft:fletcher")]
    Fletcher,
    #[serde(rename = "minecraft:leatherworker")]
    Leatherworker,
    #[serde(rename = "minecraft:librarian")]
    Librarian,
    #[serde(rename = "minecraft:mason")]
    Mason,
    #[serde(rename = "minecraft:nitwit")]
    Nitwit,
    #[serde(rename = "minecraft:shepherd")]
    Shepherd,
    #[serde(rename = "minecraft:toolsmith")]
    Toolsmith,
    #[serde(rename = "minecraft:weaponsmith")]
    Weaponsmith,
}

impl VillagerProfession {
    pub const ALL: [Self; 15] = [
        Self::None,
        Self::Armorer,
        Self::Butcher,
        Self::Cartographer,
        Self::Cleric,
        Self::Farmer,
        Self::Fisherman,
        Self::Fletcher,
        Self::Leatherworker,
        Self::Librarian,
        Self::Mason,
        Self::Nitwit,
        Self::Shepherd,
        Self::Toolsmith,
        Self::Weaponsmith,
    ];

    pub const fn protocol_id(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VillagerData {
    #[serde(rename = "type", default)]
    pub kind: VillagerType,
    #[serde(default)]
    pub profession: VillagerProfession,
    #[serde(default = "default_level")]
    pub level: i32,
}

fn default_level() -> i32 {
    1
}

impl Default for VillagerData {
    fn default() -> Self {
        Self {
            kind: VillagerType::Plains,
            profession: VillagerProfession::None,
            level: 1,
        }
    }
}
