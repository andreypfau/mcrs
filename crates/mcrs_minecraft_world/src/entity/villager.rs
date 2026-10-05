use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::Id;
use mcrs_minecraft_registry::static_rows::rows_match;
use serde::{Deserialize, Serialize};

macro_rules! professions {
    ($($(#[$meta:meta])* $variant:ident = $name:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum VillagerProfession {
            $($(#[$meta])* #[serde(rename = $name)] $variant),*
        }

        impl VillagerProfession {
            pub const ALL: [Self; [$($name),*].len()] = [$(Self::$variant),*];

            pub const fn protocol_id(self) -> u16 {
                self as u16
            }
        }

        const _: () = assert!(
            rows_match(
                &[$(($name, VillagerProfession::$variant as u16)),*],
                mcrs_minecraft_keys::villager_profession::NAMES,
                true,
            ),
            "the profession table must equal the generated villager_profession names row by row",
        );
    };
}

professions! {
    #[default]
    None = "minecraft:none",
    Armorer = "minecraft:armorer",
    Butcher = "minecraft:butcher",
    Cartographer = "minecraft:cartographer",
    Cleric = "minecraft:cleric",
    Farmer = "minecraft:farmer",
    Fisherman = "minecraft:fisherman",
    Fletcher = "minecraft:fletcher",
    Leatherworker = "minecraft:leatherworker",
    Librarian = "minecraft:librarian",
    Mason = "minecraft:mason",
    Nitwit = "minecraft:nitwit",
    Shepherd = "minecraft:shepherd",
    Toolsmith = "minecraft:toolsmith",
    Weaponsmith = "minecraft:weaponsmith",
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VillagerData {
    pub kind: Id<keys::VillagerType>,
    pub profession: VillagerProfession,
    pub level: i32,
}

impl Default for VillagerData {
    fn default() -> Self {
        Self {
            kind: keys::villager_type::PLAINS,
            profession: VillagerProfession::None,
            level: 1,
        }
    }
}
