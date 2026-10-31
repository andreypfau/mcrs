use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_registries::{Refs, Resolved, registry_value};
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_sound::Sound;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WolfVariantAssets {
    pub wild: ResourceLocation,
    pub tame: ResourceLocation,
    pub angry: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WolfVariant {
    pub assets: WolfVariantAssets,
    pub baby_assets: WolfVariantAssets,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PigVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: PigModel,
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CowVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: CowModel,
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChickenVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: ChickenModel,
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZombieNautilusVariant {
    #[serde(default, skip_serializing_if = "is_default")]
    pub model: ZombieNautilusModel,
    pub asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatVariant {
    pub asset_id: ResourceLocation,
    pub baby_asset_id: ResourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WolfSounds<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growl_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pant_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whine_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound<M>>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WolfSoundVariant<M: Refs = Resolved> {
    pub adult_sounds: WolfSounds<M>,
    pub baby_sounds: WolfSounds<M>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PigSounds<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eat_sound: Option<Sound<M>>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PigSoundVariant<M: Refs = Resolved> {
    pub adult_sounds: PigSounds<M>,
    pub baby_sounds: PigSounds<M>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatSounds<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stray_ambient_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hiss_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eat_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beg_for_food_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purr_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purreow_sound: Option<Sound<M>>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatSoundVariant<M: Refs = Resolved> {
    pub adult_sounds: CatSounds<M>,
    pub baby_sounds: CatSounds<M>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CowSoundVariant<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound<M>>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChickenSounds<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound<M>>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChickenSoundVariant<M: Refs = Resolved> {
    pub adult_sounds: ChickenSounds<M>,
    pub baby_sounds: ChickenSounds<M>,
}

mcrs_minecraft_registries::resolve_as_is!(
    WolfVariant,
    PigVariant,
    CowVariant,
    ChickenVariant,
    ZombieNautilusVariant,
    CatVariant,
    FrogVariant,
);
