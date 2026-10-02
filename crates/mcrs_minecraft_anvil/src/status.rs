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

impl ChunkStatus {
    pub const ALL: [Self; 11] = [
        Self::Empty,
        Self::StructureStarts,
        Self::StructureReferences,
        Self::NoiseBiomes,
        Self::Biomes,
        Self::Terrain,
        Self::Features,
        Self::InitializeLight,
        Self::Light,
        Self::Spawn,
        Self::Full,
    ];
}

#[cfg(test)]
mod tests {
    use super::ChunkStatus;
    use mcrs_minecraft_registry::StaticRegistryTable;
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
        let table = StaticRegistryTable::load(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/mcrs/reports/registries.json"),
        )
        .unwrap();
        let expected: Vec<String> = table
            .registry("chunk_status")
            .unwrap()
            .names()
            .iter()
            .map(ToString::to_string)
            .collect();
        let actual: Vec<String> = ChunkStatus::ALL.iter().map(serde_name).collect();
        assert_eq!(actual, expected);
        for pair in ChunkStatus::ALL.windows(2) {
            assert!(
                pair[0] < pair[1],
                "{:?} is not before {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
