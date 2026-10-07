use mcrs_minecraft_block_predicate::provider::BlockStateProvider;
use mcrs_minecraft_core::codec::{self, NonNegativeInt, default_true, is_default, is_true};
use mcrs_minecraft_core::{Direction, ResourceLocation};
use mcrs_minecraft_registry::Holder;
use mcrs_minecraft_sound::SoundEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockTransformer(
    #[serde(deserialize_with = "codec::sized_list::<1, 200, _, _>")] pub Vec<BlockTransformData>,
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockTransformData {
    pub block_state_provider: BlockStateProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<Holder<SoundEvent>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub particle: TransformParticle,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disallowed_faces: Vec<Direction>,
    // chisle: an unchecked name until loot tables load as a registry
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<ResourceLocation>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub drop_strategy: DropStrategy,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub update_from_neighbors: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub transform_type: TransformType,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub consume_on_use: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub item_damage_per_use: NonNegativeInt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformParticle {
    #[default]
    None,
    Scrape,
    WaxOn,
    WaxOff,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropStrategy {
    ClickedFace,
    #[default]
    FromMiddle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformType {
    #[default]
    SingleBlock,
    CopperChest,
}
