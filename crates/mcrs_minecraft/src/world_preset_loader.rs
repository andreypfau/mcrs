//! Dynamic asset loading for world presets and dimension types using Bevy's asset system.
//!
//! This module implements Bevy AssetLoaders for loading world preset and dimension type
//! JSON files dynamically from the assets directory at runtime, rather than embedding
//! them at compile time with include_str!.
//!
//! Asset resolution follows Minecraft's namespace:path pattern:
//! - ENV=normal resolves to "minecraft:normal"
//! - "minecraft:normal" resolves to "assets/minecraft/worldgen/world_preset/normal.json"

use crate::dimension_type::DimensionType;
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetLoader, Handle, LoadContext, LoadDirectError, UntypedAssetId,
    VisitAssetDependencies,
};
use bevy_reflect::TypePath;
use mcrs_protocol::Ident;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;
use tracing::{debug, warn};

// ============================================================================
// World Preset Asset
// ============================================================================

/// Represents a dimension entry within a world preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldPresetDimensionEntry {
    /// Reference to the dimension type (e.g., "minecraft:overworld")
    #[serde(rename = "type")]
    pub dimension_type: String,
    /// Generator configuration (kept as raw Value since we don't need to parse it)
    #[serde(default)]
    pub generator: Value,
}

/// Raw world preset JSON structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorldPresetJson {
    /// Map of dimension key to dimension entry
    pub dimensions: HashMap<String, WorldPresetDimensionEntry>,
}

/// Asset type for a loaded world preset with its dimension type dependencies.
#[derive(Debug, TypePath)]
pub struct WorldPresetAsset {
    /// Name of this preset (e.g., "normal", "flat")
    pub preset_name: String,
    /// Map of dimension key to dimension entry
    pub dimensions: HashMap<String, WorldPresetDimensionEntry>,
    /// Handles to dependent dimension type assets
    pub dimension_type_handles: HashMap<String, Handle<DimensionTypeAsset>>,
}

impl Asset for WorldPresetAsset {}

impl VisitAssetDependencies for WorldPresetAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        // Register dimension types as dependencies so they load before this asset is considered "loaded"
        for handle in self.dimension_type_handles.values() {
            visit(handle.id().untyped());
        }
    }
}

impl WorldPresetAsset {
    /// Returns an ordered list of dimension entries as (dimension_key, dimension_type_ref) tuples.
    /// Order is deterministic: sorted alphabetically by dimension key.
    pub fn ordered_dimensions(&self) -> Vec<(Ident<String>, Ident<String>)> {
        let mut dims: Vec<_> = self
            .dimensions
            .iter()
            .map(|(key, entry)| {
                let dim_key = Ident::from_str(key).expect(&format!("Invalid dimension key: {}", key));
                let dim_type = Ident::from_str(&entry.dimension_type)
                    .expect(&format!("Invalid dimension type: {}", entry.dimension_type));
                (dim_key, dim_type)
            })
            .collect();
        // Sort by dimension key for deterministic ordering
        dims.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        dims
    }
}

// ============================================================================
// Dimension Type Asset
// ============================================================================

/// Asset type for a loaded dimension type.
#[derive(Debug, Clone, TypePath)]
pub struct DimensionTypeAsset {
    /// The fully parsed dimension type data
    pub dimension_type: DimensionType,
    /// The identifier for this dimension type (e.g., "minecraft:overworld")
    pub id: Ident<String>,
}

impl Asset for DimensionTypeAsset {}

impl VisitAssetDependencies for DimensionTypeAsset {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {
        // Dimension types have no dependencies
    }
}

// ============================================================================
// World Preset Loader
// ============================================================================

#[derive(Default, TypePath)]
pub struct WorldPresetLoader;

#[derive(Debug, Error)]
pub enum WorldPresetLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(String),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

impl AssetLoader for WorldPresetLoader {
    type Asset = WorldPresetAsset;
    type Settings = ();
    type Error = WorldPresetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let preset_json: WorldPresetJson = serde_json::from_slice(&bytes)
            .map_err(|e| WorldPresetLoaderError::Json(e.to_string()))?;

        // Extract preset name from the asset path
        let preset_name = load_context
            .path()
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        debug!(
            preset = %preset_name,
            dimension_count = preset_json.dimensions.len(),
            "Loading world preset"
        );

        // Load dimension type dependencies
        let mut dimension_type_handles = HashMap::new();

        for (dim_key, entry) in &preset_json.dimensions {
            let type_ref = &entry.dimension_type;

            // Skip if we already have this dimension type
            if dimension_type_handles.contains_key(type_ref) {
                continue;
            }

            // Parse the dimension type reference to extract namespace and path
            if let Ok(ident) = Ident::<String>::from_str(type_ref) {
                let asset_path = format!(
                    "{}/dimension_type/{}.json",
                    ident.namespace(),
                    ident.path()
                );

                debug!(
                    dimension_key = %dim_key,
                    dimension_type = %type_ref,
                    asset_path = %asset_path,
                    "Loading dimension type dependency"
                );

                let handle: Handle<DimensionTypeAsset> = load_context.load(asset_path);
                dimension_type_handles.insert(type_ref.clone(), handle);
            } else {
                warn!(
                    dimension_type = %type_ref,
                    "Invalid dimension type identifier format"
                );
            }
        }

        Ok(WorldPresetAsset {
            preset_name,
            dimensions: preset_json.dimensions,
            dimension_type_handles,
        })
    }
}

// ============================================================================
// Dimension Type Loader
// ============================================================================

#[derive(Default, TypePath)]
pub struct DimensionTypeLoader;

#[derive(Debug, Error)]
pub enum DimensionTypeLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(String),
}

impl AssetLoader for DimensionTypeLoader {
    type Asset = DimensionTypeAsset;
    type Settings = ();
    type Error = DimensionTypeLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let dimension_type: DimensionType = serde_json::from_slice(&bytes)
            .map_err(|e| DimensionTypeLoaderError::Json(e.to_string()))?;

        // Construct the identifier from the asset path
        // Path format: "minecraft/dimension_type/overworld.json" -> "minecraft:overworld"
        let path = load_context.path().path();
        let id = extract_dimension_type_id(path);

        debug!(
            id = %id,
            min_y = dimension_type.min_y,
            height = dimension_type.height,
            "Loaded dimension type"
        );

        Ok(DimensionTypeAsset {
            dimension_type,
            id,
        })
    }
}

/// Extract the dimension type identifier from the asset path.
/// Path format: "minecraft/dimension_type/overworld.json" -> "minecraft:overworld"
fn extract_dimension_type_id(path: &std::path::Path) -> Ident<String> {
    let path_str = path.to_string_lossy();

    // Try to find the pattern "namespace/dimension_type/name.json"
    if let Some(dim_type_idx) = path_str.find("/dimension_type/") {
        let namespace = &path_str[..dim_type_idx];
        // Get just the namespace name (last component before dimension_type)
        let namespace = namespace.rsplit('/').next().unwrap_or("minecraft");

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("overworld");

        let id_str = format!("{}:{}", namespace, name);
        return Ident::from_str(&id_str).unwrap_or_else(|_| {
            Ident::from_str("minecraft:overworld").unwrap()
        });
    }

    // Fallback: just use the file stem
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("overworld");

    Ident::from_str(&format!("minecraft:{}", name)).unwrap_or_else(|_| {
        Ident::from_str("minecraft:overworld").unwrap()
    })
}

/// Resolve a preset name (e.g., "normal") to an asset path.
/// If the name already contains a namespace (e.g., "minecraft:normal"), use it directly.
/// Otherwise, assume "minecraft" namespace.
pub fn resolve_preset_asset_path(preset_name: &str) -> String {
    if preset_name.contains(':') {
        // Already namespaced: "minecraft:normal" -> "minecraft/worldgen/world_preset/normal.json"
        if let Ok(ident) = Ident::<String>::from_str(preset_name) {
            return format!(
                "{}/worldgen/world_preset/{}.json",
                ident.namespace(),
                ident.path()
            );
        }
    }

    // Default to minecraft namespace
    format!("minecraft/worldgen/world_preset/{}.json", preset_name)
}
