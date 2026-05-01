use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct Instrument {
    pub sound_event: String,
    pub use_duration: f32,
    pub range: f32,
    pub description: serde_json::Value,
}

impl Asset for Instrument {}

impl VisitAssetDependencies for Instrument {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct InstrumentLoader;

#[derive(Debug, thiserror::Error)]
pub enum InstrumentLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for InstrumentLoader {
    type Asset = Instrument;
    type Settings = ();
    type Error = InstrumentLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Instrument, InstrumentLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
    }

    #[test]
    fn deserialize_all_instruments() {
        let dir = assets_dir().join("minecraft/instrument");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("instrument dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<Instrument>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} entries failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no instrument files found");
        eprintln!("successfully deserialized {count} instruments");
    }
}
