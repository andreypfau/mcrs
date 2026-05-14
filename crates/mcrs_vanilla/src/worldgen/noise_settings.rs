use std::sync::Arc;

use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies,
};
use bevy_reflect::TypePath;

use crate::ResourceLocation;

/// Stub for noise generator settings.
///
/// Full deserialization will be added when the worldgen pipeline consumes
/// these settings.
#[derive(Debug, Clone, Default, TypePath)]
pub struct NoiseGeneratorSettings;

impl NoiseGeneratorSettings {
    pub fn load(
        ctx: &mut LoadContext<'_>,
        loc: &ResourceLocation<Arc<str>>,
    ) -> Handle<NoiseGeneratorSettings> {
        ctx.load(format!(
            "{}/worldgen/noise_settings/{}.json",
            loc.namespace(),
            loc.path()
        ))
    }
}

impl Asset for NoiseGeneratorSettings {}

impl VisitAssetDependencies for NoiseGeneratorSettings {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct NoiseGeneratorSettingsLoader;

impl AssetLoader for NoiseGeneratorSettingsLoader {
    type Asset = NoiseGeneratorSettings;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<NoiseGeneratorSettings, std::io::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(NoiseGeneratorSettings)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}
