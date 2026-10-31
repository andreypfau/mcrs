use mcrs_minecraft_block_predicate::provider::BlockStateProvider;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::codec::{
    self, NonNegativeInt, Validate, default_true, is_default, is_true,
};
use mcrs_minecraft_registries::{Named, Refs, Resolved, registry_value};
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_sound::Sound;
use serde::{Deserialize, Serialize};

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", transparent)]
pub struct BlockTransformer<M: Refs = Resolved>(pub Vec<BlockTransformData<M>>);

mcrs_minecraft_core::validated!(BlockTransformer<Named>);

impl<M: Refs> Validate for BlockTransformer<M> {
    fn validate(&self) -> Result<(), String> {
        codec::list_size::<1, 200>(self.0.len())
    }
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockTransformData<M: Refs = Resolved> {
    pub block_state_provider: BlockStateProvider<M>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<Sound<M>>,
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
