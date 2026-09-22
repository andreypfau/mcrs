use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use mcrs_minecraft_assets::asset::read_all;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_worldgen_feature::tree::BlockStateProvider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(transparent)]
pub struct BlockTransformer(pub Vec<BlockTransformData>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockTransformData {
    pub block_state_provider: BlockStateProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub particle: Option<TransformParticle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disallowed_faces: Vec<Direction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_strategy: Option<DropStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_from_neighbors: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_type: Option<TransformType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consume_on_use: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_damage_per_use: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformParticle {
    None,
    Scrape,
    WaxOn,
    WaxOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropStrategy {
    ClickedFace,
    FromMiddle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformType {
    SingleBlock,
    CopperChest,
}

impl Asset for BlockTransformer {}

impl VisitAssetDependencies for BlockTransformer {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct BlockTransformerLoader;

impl AssetLoader for BlockTransformerLoader {
    type Asset = BlockTransformer;
    type Settings = ();
    type Error = crate::jukebox_song::JukeboxSongLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<BlockTransformer, Self::Error> {
        let bytes = read_all(reader).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
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
            let parsed: BlockTransformer = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let original: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(serde_json::to_value(&parsed).unwrap(), original, "{}", path.display());
            count += 1;
        }
        assert_eq!(count, 3);
    }
}
