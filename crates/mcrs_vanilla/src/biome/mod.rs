pub mod climate;
pub mod source;

use std::collections::HashMap;
use std::sync::Arc;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::ResourceLocation;

/// Full biome definition matching the Minecraft 1.21+ JSON format.
///
/// This is a leaf asset with no `Handle` references, so it deserializes
/// directly without a Proto layer.
#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct Biome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    pub effects: BiomeEffects,
    #[serde(default, deserialize_with = "one_or_many")]
    pub carvers: Vec<ResourceLocation<Arc<str>>>,
    #[serde(default)]
    pub features: Vec<Vec<ResourceLocation<Arc<str>>>>,
    pub spawners: BiomeSpawners,
    #[serde(default)]
    pub spawn_costs: HashMap<ResourceLocation<Arc<str>>, SpawnCost>,
    #[serde(default)]
    pub attributes: Option<serde_json::Value>,
}

impl Biome {
    pub fn load(ctx: &mut LoadContext<'_>, loc: &ResourceLocation<Arc<str>>) -> Handle<Biome> {
        ctx.load(format!("{}/worldgen/biome/{}.json", loc.namespace(), loc.path()))
    }
}

impl Asset for Biome {}

impl VisitAssetDependencies for Biome {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Debug, Clone, Deserialize)]
pub struct BiomeEffects {
    #[serde(default)]
    pub water_color: Option<String>,
    #[serde(default)]
    pub foliage_color: Option<String>,
    #[serde(default)]
    pub grass_color: Option<String>,
    #[serde(default)]
    pub grass_color_modifier: Option<String>,
    #[serde(default)]
    pub dry_foliage_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BiomeSpawners {
    #[serde(default)]
    pub ambient: Vec<SpawnerData>,
    #[serde(default)]
    pub axolotls: Vec<SpawnerData>,
    #[serde(default)]
    pub creature: Vec<SpawnerData>,
    #[serde(default)]
    pub misc: Vec<SpawnerData>,
    #[serde(default)]
    pub monster: Vec<SpawnerData>,
    #[serde(default)]
    pub underground_water_creature: Vec<SpawnerData>,
    #[serde(default)]
    pub water_ambient: Vec<SpawnerData>,
    #[serde(default)]
    pub water_creature: Vec<SpawnerData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpawnerData {
    #[serde(rename = "type")]
    pub entity_type: ResourceLocation<Arc<str>>,
    #[serde(rename = "minCount")]
    pub min_count: u32,
    #[serde(rename = "maxCount")]
    pub max_count: u32,
    pub weight: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpawnCost {
    pub charge: f64,
    pub energy_budget: f64,
}

// ---------------------------------------------------------------------------
// Serde helper: accept either a single value or an array
// ---------------------------------------------------------------------------

fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct OneOrManyVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for OneOrManyVisitor<T> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a single value or an array")
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            Vec::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            let item = T::deserialize(de::value::StrDeserializer::new(v))?;
            Ok(vec![item])
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            let item = T::deserialize(de::value::StringDeserializer::new(v))?;
            Ok(vec![item])
        }

        fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let item = T::deserialize(de::value::MapAccessDeserializer::new(map))?;
            Ok(vec![item])
        }
    }

    deserializer.deserialize_any(OneOrManyVisitor(std::marker::PhantomData))
}

// ---------------------------------------------------------------------------
// Asset loader
// ---------------------------------------------------------------------------

#[derive(Default, TypePath)]
pub struct BiomeLoader;

#[derive(Debug, thiserror::Error)]
pub enum BiomeLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for BiomeLoader {
    type Asset = Biome;
    type Settings = ();
    type Error = BiomeLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Biome, BiomeLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let biome: Biome = serde_json::from_slice(&bytes)?;
        Ok(biome)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
    }

    #[test]
    fn deserialize_all_biomes() {
        let biome_dir = assets_dir().join("minecraft/worldgen/biome");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&biome_dir).expect("biome dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<Biome>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!("{} of {} biomes failed to deserialize", failures.len(), count + failures.len());
        }

        assert!(count > 0, "no biome files found");
        eprintln!("successfully deserialized {count} biomes");
    }

    #[test]
    fn deserialize_plains_biome() {
        let bytes = std::fs::read(
            assets_dir().join("minecraft/worldgen/biome/plains.json"),
        )
        .unwrap();
        let biome: Biome = serde_json::from_slice(&bytes).unwrap();

        assert!((biome.temperature - 0.8).abs() < f32::EPSILON);
        assert!((biome.downfall - 0.4).abs() < f32::EPSILON);
        assert!(biome.has_precipitation);
        assert_eq!(biome.carvers.len(), 3);
        assert_eq!(biome.carvers[0].as_str(), "minecraft:cave");
        assert!(!biome.spawners.creature.is_empty());
        assert!(biome.attributes.is_some());
    }
}
