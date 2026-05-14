use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

use mcrs_core::tag::key::TaggedRegistry;
use mcrs_core::tag::tag_ref::TagRef;

impl TaggedRegistry for Dialog {
    const REGISTRY_PATH: &'static str = "dialog";
}

// ── Proto (deserialization-only) ──

#[derive(Debug, thiserror::Error)]
pub enum DialogResolveError {
    #[error("invalid resource location in dialogs: {0}")]
    InvalidResourceLocation(#[from] mcrs_core::resource_location::ResourceLocationError),
}

// ── Runtime Dialog ──

/// Round-trips the dialog JSON verbatim. The vanilla 26.1 client uses a
/// dispatching codec keyed on `type` and accepts any payload shape the
/// dispatcher recognizes, so the simplest correct approach is to preserve
/// the source JSON object unchanged and let the client interpret it.
#[derive(Debug, Clone, TypePath)]
pub struct Dialog {
    pub raw: serde_json::Map<String, serde_json::Value>,
    /// Sub-asset handle for the `dialogs` tag reference (e.g.
    /// `#minecraft:pause_screen_additions`). Runtime-only; not serialized.
    pub dialogs: Option<TagRef<Dialog>>,
}

impl Serialize for Dialog {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

impl Asset for Dialog {}

impl VisitAssetDependencies for Dialog {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        if let Some(ref tag_ref) = self.dialogs {
            visit(tag_ref.handle().id().untyped());
        }
    }
}

// ── Loader ──

#[derive(Default, TypePath)]
pub struct DialogLoader;

#[derive(Debug, thiserror::Error)]
pub enum DialogLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("resolve error: {0}")]
    Resolve(#[from] DialogResolveError),
}

impl AssetLoader for DialogLoader {
    type Asset = Dialog;
    type Settings = ();
    type Error = DialogLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Dialog, DialogLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let raw: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&bytes)?;
        let dialogs = match raw.get("dialogs").and_then(|v| v.as_str()) {
            Some(s) if s.starts_with('#') => {
                let tag_str = &s[1..];
                Some(TagRef::<Dialog>::load(tag_str, load_context).map_err(DialogResolveError::from)?)
            }
            _ => None,
        };
        Ok(Dialog { raw, dialogs })
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
    fn deserialize_all_dialogs() {
        let dir = assets_dir().join("minecraft/dialog");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("dialog dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes) {
                Ok(map) => {
                    assert!(map.contains_key("type"), "{}", path.display());
                    count += 1;
                }
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} dialogs failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no dialog files found");
        eprintln!("successfully deserialized {count} dialogs");
    }
}
