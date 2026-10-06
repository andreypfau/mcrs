use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::NonNegativeInt;
use mcrs_minecraft_core::value_provider::IntProvider;
use serde::{Deserialize, Serialize};

macro_rules! mob_categories {
    ($($category:ident),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum MobCategory {
            $($category),*
        }

        impl MobCategory {
            pub const ALL: &[MobCategory] = &[$(MobCategory::$category),*];
        }
    };
}

mob_categories! {
    Monster,
    Creature,
    Ambient,
    Axolotls,
    UndergroundWaterCreature,
    WaterCreature,
    WaterAmbient,
    Misc,
}

// The weight sits beside the entry's own fields, not under `data` as in `Weighted<T>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnerData {
    #[serde(rename = "type")]
    pub entity: ResourceLocation,
    pub count: IntProvider,
    pub weight: NonNegativeInt,
}
