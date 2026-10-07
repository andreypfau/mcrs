use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_registry::{Id, LoadReport, RegistrySet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionEntry {
    #[serde(rename = "type")]
    pub dimension_type: Id<DimensionType>,
    pub generator: ChunkGenerator,
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

pub type Dimensions = BTreeMap<ResourceKey<Dimension>, DimensionEntry>;

const LEADING: [ResourceKey<Dimension, &str>; 3] = [
    mcrs_minecraft_dimension::keys::dimension::OVERWORLD,
    mcrs_minecraft_dimension::keys::dimension::THE_NETHER,
    mcrs_minecraft_dimension::keys::dimension::THE_END,
];

/// The vanilla dimension list of a save or a world preset, which must hold the overworld.
pub fn bake(
    base: &Dimensions,
    set: &RegistrySet,
    report: &mut LoadReport,
) -> Option<Vec<(ResourceKey<Dimension>, DimensionEntry)>> {
    let baked = merge(base, set, report)?;
    if !baked
        .first()
        .is_some_and(|(key, _)| *key == mcrs_minecraft_dimension::keys::dimension::OVERWORLD)
    {
        report.missing(
            mcrs_minecraft_dimension::keys::DIMENSION,
            mcrs_minecraft_dimension::keys::dimension::OVERWORLD.as_str(),
            "the dimension list has no overworld, which every world needs",
        );
        return None;
    }
    Some(baked)
}

/// `base` with the data pack's dimensions over it, the vanilla dimensions first when present.
/// The first entry is where a player with no saved dimension spawns, so the list must not be
/// empty.
pub fn bake_list(
    base: &Dimensions,
    set: &RegistrySet,
    report: &mut LoadReport,
) -> Option<Vec<(ResourceKey<Dimension>, DimensionEntry)>> {
    let baked = merge(base, set, report)?;
    if baked.is_empty() {
        report.invalid_report(format_args!(
            "{}: the dimension list is empty, so no player has a dimension to spawn in",
            mcrs_minecraft_dimension::keys::DIMENSION
        ));
        return None;
    }
    Some(baked)
}

fn merge(
    base: &Dimensions,
    set: &RegistrySet,
    report: &mut LoadReport,
) -> Option<Vec<(ResourceKey<Dimension>, DimensionEntry)>> {
    let registry = report.registry(set, mcrs_minecraft_dimension::keys::DIMENSION)?;
    let (Some(dimensions), Some(generators)) = (
        set.entries::<Dimension, Dimension>(),
        set.entries::<Dimension, ChunkGenerator>(),
    ) else {
        panic!("the data pack loader splits minecraft:dimension");
    };

    let mut merged = base.clone();
    for (id, name) in registry.iter() {
        merged.insert(
            ResourceKey::from_location(name.clone()),
            DimensionEntry::join((&dimensions[id], &generators[id])),
        );
    }

    let mut baked = Vec::with_capacity(merged.len());
    for leading in LEADING {
        if let Some(entry) = merged.remove(leading.as_str()) {
            baked.push((leading.into(), entry));
        }
    }
    baked.extend(merged);
    Some(baked)
}
