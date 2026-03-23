use std::collections::HashMap;
use std::sync::Arc;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_ecs_macros::Resource;
use bevy_reflect::TypePath;
use mcrs_core::ResourceKey;
use serde::Deserialize;

use crate::dimension::level_stem::{DimensionDefinition, ProtoDimensionEntry};

// ===========================================================================
// Runtime type
// ===========================================================================

pub const DEFAULT_WORLD_PRESET: &str = "minecraft/worldgen/world_preset/normal.json";

/// Holds the handle to the currently loading world preset.
#[derive(Resource)]
pub struct ActiveWorldPreset {
    pub handle: Handle<WorldPreset>,
}

/// A world preset defines the set of dimensions for a world.
///
/// Each dimension is a labeled sub-asset (`DimensionDefinition`) produced by
/// the `WorldPresetLoader`.
#[derive(Debug, Clone, TypePath)]
pub struct WorldPreset {
    pub dimensions: Vec<(ResourceKey<DimensionDefinition>, Handle<DimensionDefinition>)>,
}

impl Asset for WorldPreset {}

impl VisitAssetDependencies for WorldPreset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for (_, handle) in &self.dimensions {
            visit(handle.id().untyped());
        }
    }
}

// ===========================================================================
// Proto type (serde layer — private)
// ===========================================================================

#[derive(Deserialize)]
struct ProtoWorldPreset {
    dimensions: HashMap<ResourceKey<DimensionDefinition>, ProtoDimensionEntry>,
}

// ===========================================================================
// Asset loader
// ===========================================================================

#[derive(Default, TypePath)]
pub struct WorldPresetLoader;

#[derive(Debug, thiserror::Error)]
pub enum WorldPresetLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid dimension key: {0}")]
    BadDimensionKey(String),
}

impl AssetLoader for WorldPresetLoader {
    type Asset = WorldPreset;
    type Settings = ();
    type Error = WorldPresetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<WorldPreset, WorldPresetLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let proto: ProtoWorldPreset = serde_json::from_slice(&bytes)?;

        let mut dimensions = Vec::with_capacity(proto.dimensions.len());
        for (key, entry) in proto.dimensions {
            let dim_def = entry.resolve(load_context);
            let handle = load_context.add_labeled_asset(key.to_string(), dim_def);
            dimensions.push((key, handle));
        }

        Ok(WorldPreset { dimensions })
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::source::ProtoBiomeSource;
    use crate::worldgen::chunk_generator::ProtoChunkGenerator;
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
    }

    /// Deserialize every world preset JSON through the Proto layer.
    /// This validates the serde definitions without needing a full Bevy LoadContext.
    #[test]
    fn deserialize_all_world_presets() {
        let preset_dir = assets_dir().join("minecraft/worldgen/world_preset");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&preset_dir).expect("world_preset dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<ProtoWorldPreset>(&bytes) {
                Ok(proto) => {
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
                "{} of {} world presets failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no world preset files found");
        eprintln!("successfully deserialized {count} world presets");
    }

    #[test]
    fn deserialize_normal_preset() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/worldgen/world_preset/normal.json"),
        )
        .unwrap();
        let proto: ProtoWorldPreset = serde_json::from_slice(&bytes).unwrap();

        assert!(proto.dimensions.contains_key("minecraft:overworld"));
        let overworld = &proto.dimensions["minecraft:overworld"];
        assert_eq!(overworld.dimension_type.as_str(), "minecraft:overworld");
        match &overworld.generator {
            ProtoChunkGenerator::Noise(n) => {
                assert_eq!(n.settings.as_str(), "minecraft:overworld");
                match &n.biome_source {
                    ProtoBiomeSource::MultiNoise(src) => {
                        assert_eq!(
                            src.preset.as_ref().unwrap().as_str(),
                            "minecraft:overworld"
                        );
                    }
                    _ => panic!("expected MultiNoise biome source"),
                }
            }
            _ => panic!("expected Noise generator"),
        }
    }

    #[test]
    fn deserialize_flat_preset() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/worldgen/world_preset/flat.json"),
        )
        .unwrap();
        let proto: ProtoWorldPreset = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(proto.dimensions.len(), 3);
        let overworld = &proto.dimensions["minecraft:overworld"];
        match &overworld.generator {
            ProtoChunkGenerator::Flat(f) => {
                assert_eq!(f.settings.biome.as_str(), "minecraft:plains");
                assert_eq!(f.settings.layers.len(), 3);
                assert_eq!(f.settings.structure_overrides.len(), 2);
                assert!(!f.settings.features);
                assert!(!f.settings.lakes);
            }
            _ => panic!("expected Flat generator"),
        }
    }

    #[test]
    fn deserialize_single_biome_surface_preset() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/worldgen/world_preset/single_biome_surface.json"),
        )
        .unwrap();
        let proto: ProtoWorldPreset = serde_json::from_slice(&bytes).unwrap();

        let overworld = &proto.dimensions["minecraft:overworld"];
        match &overworld.generator {
            ProtoChunkGenerator::Noise(n) => match &n.biome_source {
                ProtoBiomeSource::Fixed { biome } => {
                    assert_eq!(biome.as_str(), "minecraft:plains");
                }
                _ => panic!("expected Fixed biome source"),
            },
            _ => panic!("expected Noise generator"),
        }
    }
}
