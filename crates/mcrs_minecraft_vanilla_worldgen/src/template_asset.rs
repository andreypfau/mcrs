use bevy_app::{App, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetApp, AssetLoader, LoadContext};
use bevy_reflect::TypePath;
use mcrs_minecraft_worldgen_feature::template::Template;
use mcrs_minecraft_worldgen_feature::template_file::{TemplateFileError, decode_template};
use thiserror::Error;

pub struct WorldgenAssetsPlugin;

impl Plugin for WorldgenAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<TemplateAsset>()
            .register_asset_loader(TemplateLoader);
    }
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct TemplateAsset {
    pub template: Template,
}

#[derive(Debug, Error)]
pub enum TemplateLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{path}: {source}")]
    Template {
        path: String,
        source: TemplateFileError,
    },
}

#[derive(Default, TypePath)]
pub struct TemplateLoader;

impl AssetLoader for TemplateLoader {
    type Asset = TemplateAsset;
    type Settings = ();
    type Error = TemplateLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let template = decode_template(&bytes).map_err(|source| TemplateLoaderError::Template {
            path: load_context.path().to_string(),
            source,
        })?;
        Ok(TemplateAsset { template })
    }

    fn extensions(&self) -> &[&str] {
        &["nbt"]
    }
}
