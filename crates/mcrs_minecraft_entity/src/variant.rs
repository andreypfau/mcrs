use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_registry::Holder;
use mcrs_minecraft_sound::SoundEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WolfVariantAssets {
    pub wild: ResourceLocation,
    pub tame: ResourceLocation,
    pub angry: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WolfVariant {
    pub assets: WolfVariantAssets,
    pub baby_assets: WolfVariantAssets,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PigVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: PigModel,
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CowVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: CowModel,
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChickenVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: ChickenModel,
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZombieNautilusVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: ZombieNautilusModel,
    pub asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatVariant {
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrogVariant {
    pub asset_id: ResourceLocation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PigModel {
    #[default]
    Normal,
    Cold,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CowModel {
    #[default]
    Normal,
    Cold,
    Warm,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChickenModel {
    #[default]
    Normal,
    Cold,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZombieNautilusModel {
    #[default]
    Normal,
    Warm,
}

type Sound = Holder<SoundEvent>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WolfSounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growl_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pant_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whine_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WolfSoundVariant {
    pub adult_sounds: WolfSounds,
    pub baby_sounds: WolfSounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PigSounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eat_sound: Option<Sound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PigSoundVariant {
    pub adult_sounds: PigSounds,
    pub baby_sounds: PigSounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatSounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stray_ambient_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hiss_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eat_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beg_for_food_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purr_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purreow_sound: Option<Sound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatSoundVariant {
    pub adult_sounds: CatSounds,
    pub baby_sounds: CatSounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CowSoundVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChickenSounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChickenSoundVariant {
    pub adult_sounds: ChickenSounds,
    pub baby_sounds: ChickenSounds,
}
