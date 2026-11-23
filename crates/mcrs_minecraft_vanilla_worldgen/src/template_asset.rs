use bevy_app::{App, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetApp, AssetLoader, LoadContext};
use bevy_reflect::TypePath;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_worldgen_feature::template::Template;
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
    #[error(transparent)]
    Nbt(#[from] mcrs_minecraft_nbt::Error),
    #[error("{path}: DataVersion {found}, expected {expected}")]
    DataVersion {
        path: String,
        found: i32,
        expected: i32,
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
        let template =
            mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes::<Template, _>(bytes.as_slice())?;
        if template.data_version != VERSION.world_version {
            return Err(TemplateLoaderError::DataVersion {
                path: load_context.path().to_string(),
                found: template.data_version,
                expected: VERSION.world_version,
            });
        }
        Ok(TemplateAsset { template })
    }

    fn extensions(&self) -> &[&str] {
        &["nbt"]
    }
}
