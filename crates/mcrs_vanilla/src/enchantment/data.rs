use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

use crate::item::Item;
use mcrs_core::tag::tag_ref::TagRef;

// ── Raw (deserialization-only, also used by LoadedEnchantments) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: String,
    pub weight: u32,
    pub max_level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_items: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}

// ── Resolve error ──

#[derive(Debug, thiserror::Error)]
pub enum EnchantmentDataResolveError {
    #[error("tag reference field `{0}` does not start with '#'")]
    MissingHashPrefix(String),
    #[error("invalid resource location in tag reference: {0}")]
    InvalidResourceLocation(#[from] mcrs_core::resource_location::ResourceLocationError),
}

impl RawEnchantmentData {
    fn resolve(
        self,
        load_context: &mut LoadContext<'_>,
    ) -> Result<EnchantmentData, EnchantmentDataResolveError> {
        let supported_tag_str = self
            .supported_items
            .strip_prefix('#')
            .ok_or_else(|| {
                EnchantmentDataResolveError::MissingHashPrefix(self.supported_items.clone())
            })?;
        let supported_items = TagRef::<Item>::load(supported_tag_str, load_context)?;

        let primary_items = self
            .primary_items
            .map(|s| {
                let tag_str = s
                    .strip_prefix('#')
                    .ok_or_else(|| EnchantmentDataResolveError::MissingHashPrefix(s.clone()))?;
                TagRef::<Item>::load(tag_str, load_context)
                    .map_err(EnchantmentDataResolveError::from)
            })
            .transpose()?;

        let exclusive_set = self
            .exclusive_set
            .map(|s| {
                let tag_str = s
                    .strip_prefix('#')
                    .ok_or_else(|| EnchantmentDataResolveError::MissingHashPrefix(s.clone()))?;
                TagRef::<EnchantmentData>::load(tag_str, load_context)
                    .map_err(EnchantmentDataResolveError::from)
            })
            .transpose()?;

        Ok(EnchantmentData {
            description: self.description,
            min_cost: self.min_cost,
            max_cost: self.max_cost,
            anvil_cost: self.anvil_cost,
            slots: self.slots,
            supported_items,
            weight: self.weight,
            max_level: self.max_level,
            primary_items,
            exclusive_set,
            effects: self.effects,
        })
    }
}

// ── Runtime EnchantmentData ──

#[derive(Debug, Clone, TypePath)]
pub struct EnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: TagRef<Item>,
    pub weight: u32,
    pub max_level: u32,
    pub primary_items: Option<TagRef<Item>>,
    pub exclusive_set: Option<TagRef<EnchantmentData>>,
    pub effects: Option<serde_json::Value>,
}

impl Asset for EnchantmentData {}

impl VisitAssetDependencies for EnchantmentData {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.supported_items.handle().id().untyped());
        if let Some(ref primary) = self.primary_items {
            visit(primary.handle().id().untyped());
        }
        if let Some(ref exc) = self.exclusive_set {
            visit(exc.handle().id().untyped());
        }
    }
}

// ── Loader ──

#[derive(Default, TypePath)]
pub struct EnchantmentDataLoader;

#[derive(Debug, thiserror::Error)]
pub enum EnchantmentDataLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("resolve error: {0}")]
    Resolve(#[from] EnchantmentDataResolveError),
}

impl AssetLoader for EnchantmentDataLoader {
    type Asset = EnchantmentData;
    type Settings = ();
    type Error = EnchantmentDataLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<EnchantmentData, EnchantmentDataLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let raw: RawEnchantmentData = serde_json::from_slice(&bytes)?;
        Ok(raw.resolve(load_context)?)
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
    fn deserialize_all_enchantments() {
        let dir = assets_dir().join("minecraft/enchantment");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("enchantment dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<RawEnchantmentData>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} enchantments failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no enchantment files found");
        eprintln!("successfully deserialized {count} enchantments");
    }
}
