use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::rl;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ChunkStatus {
    #[serde(rename = "minecraft:empty")]
    Empty,
    #[serde(rename = "minecraft:structure_starts")]
    StructureStarts,
    #[serde(rename = "minecraft:structure_references")]
    StructureReferences,
    #[serde(rename = "minecraft:noise_biomes")]
    NoiseBiomes,
    #[serde(rename = "minecraft:biomes")]
    Biomes,
    #[serde(rename = "minecraft:terrain")]
    Terrain,
    #[serde(rename = "minecraft:features")]
    Features,
    #[serde(rename = "minecraft:initialize_light")]
    InitializeLight,
    #[serde(rename = "minecraft:light")]
    Light,
    #[serde(rename = "minecraft:spawn")]
    Spawn,
    #[serde(rename = "minecraft:full")]
    Full,
}

impl RegistryKey for ChunkStatus {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:chunk_status");
}

#[cfg(test)]
mod tests {
    use super::ChunkStatus;
    use mcrs_minecraft_registry::static_report::from_report;
    use std::path::PathBuf;

    fn serde_name(status: &ChunkStatus) -> String {
        serde_json::to_value(status)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn the_statuses_are_the_registry_in_names_and_order() {
        let set = from_report(
            &std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets/mcrs/reports/registries.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let expected: Vec<String> = set
            .table("minecraft:chunk_status")
            .unwrap()
            .names()
            .iter()
            .map(ToString::to_string)
            .collect();
        let statuses: Vec<ChunkStatus> = expected
            .iter()
            .map(|name| {
                serde_json::from_value(serde_json::Value::String(name.clone()))
                    .unwrap_or_else(|error| panic!("{name} is no chunk status: {error}"))
            })
            .collect();
        let actual: Vec<String> = statuses.iter().map(serde_name).collect();
        assert_eq!(actual, expected);
        for pair in statuses.windows(2) {
            assert!(
                pair[0] < pair[1],
                "{:?} is not before {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
