use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Id, LoadReport, RegistrySet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionEntry {
    #[serde(rename = "type")]
    pub dimension_type: Id<keys::DimensionType>,
    pub generator: ChunkGenerator,
}

pub type Dimensions = BTreeMap<ResourceKey<keys::Dimension>, DimensionEntry>;

const LEADING: [ResourceKey<keys::Dimension, &str>; 3] = [
    keys::dimension::OVERWORLD,
    keys::dimension::THE_NETHER,
    keys::dimension::THE_END,
];

pub fn bake(
    base: &Dimensions,
    set: &RegistrySet,
    report: &mut LoadReport,
) -> Option<Vec<(ResourceKey<keys::Dimension>, DimensionEntry)>> {
    let registry = report.registry(set, keys::DIMENSION)?;
    let defined = set
        .entries::<keys::Dimension, DimensionEntry>()
        .expect("the data pack loader parses minecraft:dimension");

    let mut merged = base.clone();
    for id in registry.ids() {
        let name = registry.name(id).expect("an id of the registry has a name");
        merged.insert(
            ResourceKey::from_location(name.clone()),
            defined[id].clone(),
        );
    }

    let mut baked = Vec::with_capacity(merged.len());
    for leading in LEADING {
        if let Some(entry) = merged.remove(leading.as_str()) {
            baked.push((leading.into(), entry));
        }
    }
    baked.extend(merged);

    if !baked
        .first()
        .is_some_and(|(key, _)| *key == keys::dimension::OVERWORLD)
    {
        report.missing(
            keys::DIMENSION,
            keys::dimension::OVERWORLD.as_str(),
            "the dimension list has no overworld, which every world needs",
        );
        return None;
    }
    Some(baked)
}
