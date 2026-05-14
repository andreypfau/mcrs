use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

/// Unit-shaped registry entry. Vanilla `WorldClock.DIRECT_CODEC` is
/// `MapCodec.unitCodec(WorldClock::new)`, so the wire payload is an
/// empty NBT compound.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TypePath)]
pub struct WorldClock {}

impl Asset for WorldClock {}

impl VisitAssetDependencies for WorldClock {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct WorldClockLoader;

#[derive(Debug, thiserror::Error)]
pub enum WorldClockLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for WorldClockLoader {
    type Asset = WorldClock;
    type Settings = ();
    type Error = WorldClockLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<WorldClock, WorldClockLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        if bytes.iter().all(|b| b.is_ascii_whitespace()) {
            return Ok(WorldClock {});
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}
