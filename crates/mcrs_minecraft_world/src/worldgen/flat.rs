use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ResourceLocation;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_registry::{Id, Registry};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatChunkGenerator {
    pub settings: FlatLevelGeneratorSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FlatSettingsFields")]
pub struct FlatLevelGeneratorSettings {
    pub biome: Id<Biome>,
    pub features: bool,
    pub lakes: bool,
    pub layers: Vec<FlatLayerInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub structure_overrides: Vec<ResourceLocation<Arc<str>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatLayerInfo {
    // chisle: block and structure set names are not checked against their registries; upgrade = Id<keys::Block> and HolderSet<StructureSet>.
    pub block: ResourceLocation<Arc<str>>,
    pub height: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlatSettingsFields {
    biome: Option<Id<Biome>>,
    #[serde(default)]
    features: bool,
    #[serde(default)]
    lakes: bool,
    layers: Vec<FlatLayerInfo>,
    #[serde(default)]
    structure_overrides: Vec<ResourceLocation<Arc<str>>>,
}

impl TryFrom<FlatSettingsFields> for FlatLevelGeneratorSettings {
    type Error = String;

    fn try_from(fields: FlatSettingsFields) -> Result<Self, String> {
        let biome = match fields.biome {
            Some(biome) => biome,
            None => Registry::<Biome>::in_scope("the flat generator's default biome", |biomes| {
                biomes.require(&mcrs_minecraft_biome::keys::biome::PLAINS)
            })
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?,
        };
        Ok(FlatLevelGeneratorSettings {
            biome,
            features: fields.features,
            lakes: fields.lakes,
            layers: fields.layers,
            structure_overrides: fields.structure_overrides,
        })
    }
}
