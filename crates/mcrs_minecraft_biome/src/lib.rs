pub mod beta_surface;
pub mod climate;
pub mod overworld_preset;
pub mod source;
pub mod zoom;

use std::sync::Arc;

use bevy_asset::{Asset, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_environment::attribute::{EnvironmentAttributeMap, MobSpawnSettings};
use mcrs_minecraft_worldgen_feature::FeatureStepList;

pub use mcrs_minecraft_worldgen_structure::{MobCategory, SpawnerData};

pub const NATURAL_MOB_SPAWNS: &str = "minecraft:gameplay/natural_mob_spawns";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureModifier {
    None,
    Frozen,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct Biome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<TemperatureModifier>,
    pub effects: BiomeEffects,
    #[serde(default)]
    pub attributes: EnvironmentAttributeMap,
    #[serde(default, deserialize_with = "one_or_many")]
    pub carvers: Vec<ResourceLocation<Arc<str>>>,
    #[serde(default)]
    pub features: Vec<FeatureStepList>,
}

impl mcrs_minecraft_core::tag_key::TaggedRegistry for Biome {
    const REGISTRY_PATH: &'static str = "worldgen/biome";
}

impl Biome {
    pub fn load(ctx: &mut LoadContext<'_>, loc: &ResourceLocation<Arc<str>>) -> Handle<Biome> {
        ctx.load(format!(
            "{}/worldgen/biome/{}.json",
            loc.namespace(),
            loc.path()
        ))
    }

    pub fn natural_mob_spawns(&self) -> serde_json::Result<Option<MobSpawnSettings>> {
        self.attributes.argument(NATURAL_MOB_SPAWNS)
    }
}

/// Biome data subset for NETWORK_CODEC — omits server-only generation settings.
///
/// Sent to clients during Configuration; excludes carvers, features, and the
/// attributes the client is not allowed to see.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkBiome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<TemperatureModifier>,
    pub attributes: EnvironmentAttributeMap,
    pub effects: BiomeEffects,
}

impl From<&Biome> for NetworkBiome {
    fn from(biome: &Biome) -> Self {
        NetworkBiome {
            temperature: biome.temperature,
            downfall: biome.downfall,
            has_precipitation: biome.has_precipitation,
            temperature_modifier: biome.temperature_modifier,
            attributes: biome.attributes.filter_syncable(),
            effects: biome.effects.clone(),
        }
    }
}

impl Asset for Biome {}

impl VisitAssetDependencies for Biome {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_testing::assets_dir;

    #[test]
    fn deserialize_all_biomes() {
        for (path, biome) in
            mcrs_minecraft_worldgen_testing::parse_all::<Biome>("minecraft/worldgen/biome")
        {
            let raw: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let attributes = raw
                .get("attributes")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            assert_eq!(
                serde_json::to_value(&biome.attributes).unwrap(),
                attributes,
                "{} attributes must round-trip unchanged",
                path.display()
            );
        }
    }

    #[test]
    fn network_biome_omits_server_fields() {
        let bytes =
            std::fs::read(assets_dir().join("minecraft/worldgen/biome/plains.json")).unwrap();
        let biome: Biome = serde_json::from_slice(&bytes).unwrap();
        let network = NetworkBiome::from(&biome);

        let json = serde_json::to_value(&network).unwrap();
        assert!(json.get("temperature").is_some());
        assert!(json.get("downfall").is_some());
        assert!(json.get("has_precipitation").is_some());
        assert!(json.get("effects").is_some());
        assert!(json.get("carvers").is_none());
        assert!(json.get("features").is_none());

        let attributes = json.get("attributes").expect("attributes are synced");
        assert_eq!(
            attributes.get("minecraft:visual/sky_color").unwrap(),
            "#78a7ff"
        );
        assert!(
            attributes.get(NATURAL_MOB_SPAWNS).is_none(),
            "spawns are server-only"
        );

        let nbt = mcrs_minecraft_nbt::to_nbt_compound(&network)
            .expect("network biome must encode to NBT");
        let Some(mcrs_minecraft_nbt::tag::NbtTag::Compound(attributes)) = nbt.get("attributes")
        else {
            panic!("attributes must reach the client as a compound");
        };
        assert_eq!(
            attributes.get("minecraft:visual/sky_color"),
            Some(&mcrs_minecraft_nbt::tag::NbtTag::String(
                "#78a7ff".to_string()
            ))
        );

        assert!((network.temperature - biome.temperature).abs() < f32::EPSILON);
        assert!((network.downfall - biome.downfall).abs() < f32::EPSILON);
        assert_eq!(network.has_precipitation, biome.has_precipitation);
    }

    #[test]
    fn deserialize_plains_biome() {
        let bytes =
            std::fs::read(assets_dir().join("minecraft/worldgen/biome/plains.json")).unwrap();
        let biome: Biome = serde_json::from_slice(&bytes).unwrap();

        assert!((biome.temperature - 0.8).abs() < f32::EPSILON);
        assert!((biome.downfall - 0.4).abs() < f32::EPSILON);
        assert!(biome.has_precipitation);
        assert_eq!(biome.carvers.len(), 3);
        assert_eq!(biome.carvers[0].as_str(), "minecraft:cave");
        let spawns = biome
            .natural_mob_spawns()
            .unwrap()
            .expect("plains has spawns");
        assert!(!spawns.spawns_by_category[&MobCategory::Creature].is_empty());
        assert_eq!(
            biome.attributes.get(NATURAL_MOB_SPAWNS).unwrap().modifier,
            mcrs_minecraft_environment::attribute::Operation::Overlay
        );
        assert_eq!(
            biome
                .attributes
                .get("minecraft:visual/sky_color")
                .unwrap()
                .argument,
            serde_json::json!("#78a7ff")
        );
    }
}
