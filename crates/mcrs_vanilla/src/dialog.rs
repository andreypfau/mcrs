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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProtoDialog {
    #[serde(rename = "type")]
    pub dialog_type: String,
    pub title: serde_json::Value,
    #[serde(default)]
    pub external_title: Option<serde_json::Value>,
    #[serde(default)]
    pub exit_action: Option<serde_json::Value>,
    #[serde(default)]
    pub columns: Option<u32>,
    #[serde(default)]
    pub button_width: Option<u32>,
    #[serde(default)]
    pub dialogs: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DialogResolveError {
    #[error("dialogs field `{0}` does not start with '#'")]
    MissingHashPrefix(String),
    #[error("invalid resource location in dialogs: {0}")]
    InvalidResourceLocation(#[from] mcrs_core::resource_location::ResourceLocationError),
}

impl ProtoDialog {
    fn resolve(
        self,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Dialog, DialogResolveError> {
        let dialogs = self
            .dialogs
            .map(|s| {
                let tag_str = s
                    .strip_prefix('#')
                    .ok_or_else(|| DialogResolveError::MissingHashPrefix(s.clone()))?;
                TagRef::<Dialog>::load(tag_str, load_context)
                    .map_err(DialogResolveError::from)
            })
            .transpose()?;

        Ok(Dialog {
            dialog_type: self.dialog_type,
            title: self.title,
            external_title: self.external_title,
            exit_action: self.exit_action,
            columns: self.columns,
            button_width: self.button_width,
            dialogs,
        })
    }
}

// ── Runtime Dialog ──

#[derive(Debug, Clone, Serialize, TypePath)]
pub struct Dialog {
    pub dialog_type: String,
    pub title: serde_json::Value,
    pub external_title: Option<serde_json::Value>,
    pub exit_action: Option<serde_json::Value>,
    pub columns: Option<u32>,
    pub button_width: Option<u32>,
    // TagRef<Dialog> is a runtime-only Handle wrapper; not part of the network payload
    #[serde(skip_serializing)]
    pub dialogs: Option<TagRef<Dialog>>,
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
        let proto: ProtoDialog = serde_json::from_slice(&bytes)?;
        Ok(proto.resolve(load_context)?)
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
            match serde_json::from_slice::<ProtoDialog>(&bytes) {
                Ok(_) => count += 1,
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
