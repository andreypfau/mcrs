use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_registries::{Refs, Registries, Resolved, registry_value};
use mcrs_minecraft_resource::{ResourceKey, ResourceKeyBuf};

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionEntry<M: Refs = Resolved> {
    #[serde(rename = "type")]
    pub dimension_type: M::Id<DimensionType>,
    pub generator: ChunkGenerator<M>,
}

impl mcrs_minecraft_resource::RegistryValue for DimensionEntry {
    type Registry = mcrs_minecraft_dimension::Dimension;
}

impl DimensionEntry {
    pub fn split(&self) -> (Dimension, ChunkGenerator) {
        let dimension = Dimension {
            dimension_type: self.dimension_type,
        };
        (dimension, self.generator.clone())
    }

    pub fn join((dimension, generator): (&Dimension, &ChunkGenerator)) -> Self {
        DimensionEntry {
            dimension_type: dimension.dimension_type,
            generator: generator.clone(),
        }
    }
}

pub type Dimensions = BTreeMap<ResourceKeyBuf<Dimension>, DimensionEntry>;

const LEADING: [ResourceKey<Dimension, &str>; 3] = [
    mcrs_minecraft_dimension::keys::dimension::OVERWORLD,
    mcrs_minecraft_dimension::keys::dimension::THE_NETHER,
    mcrs_minecraft_dimension::keys::dimension::THE_END,
];

/// The vanilla dimension list of a save or a world preset, which must hold the overworld.
pub fn bake(
    base: &Dimensions,
    registries: &Registries,
) -> Result<Vec<(ResourceKeyBuf<Dimension>, DimensionEntry)>, String> {
    let baked = merge(base, registries)?;
    if !baked
        .first()
        .is_some_and(|(key, _)| *key == mcrs_minecraft_dimension::keys::dimension::OVERWORLD)
    {
        return Err(format!(
            "{}: the dimension list has no {}, which every world needs",
            mcrs_minecraft_dimension::keys::DIMENSION,
            mcrs_minecraft_dimension::keys::dimension::OVERWORLD.as_str()
        ));
    }
    Ok(baked)
}

/// `base` with the data pack's dimensions over it, the vanilla dimensions first when present.
/// The first entry is where a player with no saved dimension spawns, so the list must not be
/// empty.
pub fn bake_list(
    base: &Dimensions,
    registries: &Registries,
) -> Result<Vec<(ResourceKeyBuf<Dimension>, DimensionEntry)>, String> {
    let baked = merge(base, registries)?;
    if baked.is_empty() {
        return Err(format!(
            "{}: the dimension list is empty, so no player has a dimension to spawn in",
            mcrs_minecraft_dimension::keys::DIMENSION
        ));
    }
    Ok(baked)
}

fn merge(
    base: &Dimensions,
    registries: &Registries,
) -> Result<Vec<(ResourceKeyBuf<Dimension>, DimensionEntry)>, String> {
    let registry = registries.registry::<DimensionEntry>().ok_or_else(|| {
        format!(
            "{} is not loaded",
            mcrs_minecraft_dimension::keys::DIMENSION
        )
    })?;

    let mut merged = base.clone();
    for (_, name, entry) in registry.iter() {
        merged.insert(ResourceKeyBuf::from_location(name.clone()), entry.clone());
    }

    let mut baked = Vec::with_capacity(merged.len());
    for leading in LEADING {
        if let Some(entry) = merged.remove(leading.as_str()) {
            baked.push((leading.into(), entry));
        }
    }
    baked.extend(merged);
    Ok(baked)
}
