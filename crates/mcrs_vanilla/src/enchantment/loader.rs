use super::data::EnchantmentData;
use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};
use bevy_reflect::TypePath;

#[derive(Default, TypePath)]
pub struct EnchantmentDataLoader;

#[derive(Debug, thiserror::Error)]
pub enum EnchantmentLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for EnchantmentDataLoader {
    type Asset = EnchantmentData;
    type Settings = ();
    type Error = EnchantmentLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<EnchantmentData, EnchantmentLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[] // no extension claim — always use typed load::<EnchantmentData>()
    }
}
