use std::collections::HashMap;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_ecs::resource::Resource;
use bevy_reflect::TypePath;
use mcrs_minecraft_assets::asset::read_all;
use mcrs_minecraft_core::ResourceKey;
use serde::Deserialize;

use crate::dimension::{DimensionDefinition, ProtoDimensionEntry};

// ===========================================================================
// Runtime type
// ===========================================================================

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
    pub dimensions: Vec<(
        ResourceKey<DimensionDefinition>,
        Handle<DimensionDefinition>,
    )>,
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
        let bytes = read_all(reader).await?;
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
    use crate::worldgen::chunk_generator::ProtoChunkGenerator;
    use mcrs_minecraft_biome::source::ProtoBiomeSource;
    use mcrs_minecraft_worldgen_testing::assets_dir;

    #[test]
    fn deserialize_all_world_presets() {
        mcrs_minecraft_worldgen_testing::parse_all::<ProtoWorldPreset>(
            "minecraft/worldgen/world_preset",
        );
    }

    #[test]
    fn deserialize_normal_preset() {
        let bytes = std::fs::read(assets_dir().join("minecraft/worldgen/world_preset/normal.json"))
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
                        assert_eq!(src.preset.as_ref().unwrap().as_str(), "minecraft:overworld");
                    }
                    _ => panic!("expected MultiNoise biome source"),
                }
            }
            _ => panic!("expected Noise generator"),
        }
    }

    #[test]
    fn deserialize_flat_preset() {
        let bytes =
            std::fs::read(assets_dir().join("minecraft/worldgen/world_preset/flat.json")).unwrap();
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
