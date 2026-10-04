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

    #[test]
    fn every_preset_dispatches_its_generator_and_biome_source() {
        let presets: HashMap<String, ProtoWorldPreset> =
            mcrs_minecraft_worldgen_testing::parse_all::<ProtoWorldPreset>(
                "minecraft/worldgen/world_preset",
            )
            .into_iter()
            .map(|(path, proto)| {
                (
                    path.file_stem().unwrap().to_string_lossy().into_owned(),
                    proto,
                )
            })
            .collect();
        let overworld = |name: &str| &presets[name].dimensions["minecraft:overworld"].generator;

        assert!(matches!(
            overworld("normal"),
            ProtoChunkGenerator::Noise(n)
                if matches!(&n.biome_source, ProtoBiomeSource::MultiNoise(src)
                    if src.preset.as_ref().unwrap().as_str() == "minecraft:overworld")
        ));
        assert!(matches!(
            overworld("single_biome_surface"),
            ProtoChunkGenerator::Noise(n)
                if matches!(&n.biome_source, ProtoBiomeSource::Fixed { biome }
                    if biome.as_str() == "minecraft:plains")
        ));
        let ProtoChunkGenerator::Flat(flat) = overworld("flat") else {
            panic!("expected Flat generator");
        };
        assert_eq!(presets["flat"].dimensions.len(), 3);
        assert_eq!(flat.settings.layers.len(), 3);
        assert_eq!(flat.settings.structure_overrides.len(), 2);
    }
}
