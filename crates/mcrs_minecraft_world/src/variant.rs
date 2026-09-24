use bevy_asset::Asset;
use bevy_reflect::TypePath;
use mcrs_minecraft_worldgen_feature::spawn_condition::SpawnSelector;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfVariantAssets {
    pub wild: String,
    pub tame: String,
    pub angry: String,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfVariant {
    pub assets: WolfVariantAssets,
    pub baby_assets: WolfVariantAssets,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Vec<SpawnSelector>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfSounds {
    pub ambient_sound: String,
    pub death_sound: String,
    pub growl_sound: String,
    pub hurt_sound: String,
    pub pant_sound: String,
    pub whine_sound: String,
    pub step_sound: String,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfSoundVariant {
    pub adult_sounds: WolfSounds,
    pub baby_sounds: WolfSounds,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct PigSounds {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub eat_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub step_sound: Option<String>,
    #[serde(default)]
    pub saddle_sound: Option<String>,
    #[serde(default)]
    pub boost_sound: Option<String>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct PigSoundVariant {
    pub adult_sounds: PigSounds,
    pub baby_sounds: PigSounds,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CatSounds {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub beg_for_food_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub eat_sound: Option<String>,
    #[serde(default)]
    pub hiss_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub purr_sound: Option<String>,
    #[serde(default)]
    pub purreow_sound: Option<String>,
    #[serde(default)]
    pub stray_ambient_sound: Option<String>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CatSoundVariant {
    pub adult_sounds: CatSounds,
    pub baby_sounds: CatSounds,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CowSoundVariant {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub step_sound: Option<String>,
    #[serde(default)]
    pub milk_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ChickenSounds {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub step_sound: Option<String>,
    #[serde(default)]
    pub egg_sound: Option<String>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ChickenSoundVariant {
    pub adult_sounds: ChickenSounds,
    pub baby_sounds: ChickenSounds,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct PigVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Vec<SpawnSelector>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct FrogVariant {
    pub asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Vec<SpawnSelector>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CatVariant {
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Vec<SpawnSelector>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CowVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Vec<SpawnSelector>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ChickenVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Vec<SpawnSelector>,
}

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ZombieNautilusVariant {
    pub asset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Vec<SpawnSelector>,
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! deserialize_all_test {
        ($test_name:ident, $ty:ty, $dir:expr) => {
            #[test]
            fn $test_name() {
                mcrs_minecraft_worldgen_testing::parse_all::<$ty>($dir);
            }
        };
    }

    deserialize_all_test!(
        deserialize_all_wolf_variants,
        WolfVariant,
        "minecraft/wolf_variant"
    );
    deserialize_all_test!(
        deserialize_all_wolf_sound_variants,
        WolfSoundVariant,
        "minecraft/wolf_sound_variant"
    );
    deserialize_all_test!(
        deserialize_all_pig_variants,
        PigVariant,
        "minecraft/pig_variant"
    );
    deserialize_all_test!(
        deserialize_all_frog_variants,
        FrogVariant,
        "minecraft/frog_variant"
    );
    deserialize_all_test!(
        deserialize_all_cat_variants,
        CatVariant,
        "minecraft/cat_variant"
    );
    deserialize_all_test!(
        deserialize_all_cow_variants,
        CowVariant,
        "minecraft/cow_variant"
    );
    deserialize_all_test!(
        deserialize_all_chicken_variants,
        ChickenVariant,
        "minecraft/chicken_variant"
    );
    deserialize_all_test!(
        deserialize_all_zombie_nautilus_variants,
        ZombieNautilusVariant,
        "minecraft/zombie_nautilus_variant"
    );
}
