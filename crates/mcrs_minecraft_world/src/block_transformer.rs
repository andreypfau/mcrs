use mcrs_minecraft_core::codec::{NonNegativeInt, default_true, is_default};
use mcrs_minecraft_core::{Direction, ResourceLocation};
use mcrs_minecraft_item::SoundEvent;
use mcrs_minecraft_registry::Holder;
use mcrs_minecraft_worldgen_feature::tree::BlockStateProvider;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BlockTransformer(pub Vec<BlockTransformData>);

impl BlockTransformer {
    const TRANSFORMS: std::ops::RangeInclusive<usize> = 1..=200;
}

impl<'de> Deserialize<'de> for BlockTransformer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let transforms = Vec::<BlockTransformData>::deserialize(deserializer)?;
        if !Self::TRANSFORMS.contains(&transforms.len()) {
            return Err(D::Error::custom(format_args!(
                "expected between {} and {} transforms, got {}",
                Self::TRANSFORMS.start(),
                Self::TRANSFORMS.end(),
                transforms.len()
            )));
        }
        Ok(Self(transforms))
    }
}

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

fn is_true(value: &bool) -> bool {
    *value
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_round_trips() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/minecraft/block_transformer");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let parsed: BlockTransformer =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let original: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                serde_json::to_value(&parsed).unwrap(),
                original,
                "{}",
                path.display()
            );
            count += 1;
        }
        assert_eq!(count, 3);
    }
}
