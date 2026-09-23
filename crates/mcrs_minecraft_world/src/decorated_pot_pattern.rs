use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use mcrs_minecraft_assets::asset::read_all;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(deny_unknown_fields)]
pub struct DecoratedPotPattern {
    pub asset_id: String,
}

impl Asset for DecoratedPotPattern {}

impl VisitAssetDependencies for DecoratedPotPattern {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct DecoratedPotPatternLoader;

impl AssetLoader for DecoratedPotPatternLoader {
    type Asset = DecoratedPotPattern;
    type Settings = ();
    type Error = crate::jukebox_song::JukeboxSongLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<DecoratedPotPattern, Self::Error> {
        let bytes = read_all(reader).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}
