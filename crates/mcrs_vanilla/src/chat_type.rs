use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct ChatType {
    pub chat: ChatDecoration,
    pub narration: ChatDecoration,
    #[serde(default)]
    pub overlay: Option<ChatDecoration>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatDecoration {
    pub translation_key: String,
    pub parameters: Vec<String>,
    #[serde(default)]
    pub style: Option<serde_json::Value>,
}

impl Asset for ChatType {}

impl VisitAssetDependencies for ChatType {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct ChatTypeLoader;

#[derive(Debug, thiserror::Error)]
pub enum ChatTypeLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for ChatTypeLoader {
    type Asset = ChatType;
    type Settings = ();
    type Error = ChatTypeLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<ChatType, ChatTypeLoaderError> {
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
    fn deserialize_all_chat_types() {
        let dir = assets_dir().join("minecraft/chat_type");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("chat_type dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<ChatType>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} chat types failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no chat_type files found");
        eprintln!("successfully deserialized {count} chat types");
    }
}
