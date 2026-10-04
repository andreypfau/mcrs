pub mod beta_surface;
pub mod climate;
pub mod overworld_preset;
pub mod source;
pub mod zoom;

use std::sync::Arc;

use bevy_asset::{Asset, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mcrs_minecraft_core::codec::{HexRgb, is_default};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation, StaticResourceLocation};
use mcrs_minecraft_environment::attribute::id::{self, Attribute};
use mcrs_minecraft_environment::attribute::{EnvironmentAttributeMap, MobSpawnSettings, Operation};
use mcrs_minecraft_registry::key::Carver;
use mcrs_minecraft_worldgen_feature::FeatureStepList;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;
use mcrs_minecraft_worldgen_structure::DecorationStep;

pub use mcrs_minecraft_worldgen_structure::{MobCategory, SpawnerData};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureModifier {
    None,
    Frozen,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TypePath)]
pub struct Biome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<TemperatureModifier>,
    pub effects: BiomeEffects,
    #[serde(default)]
    pub attributes: EnvironmentAttributeMap,
    #[serde(
        default,
        deserialize_with = "one_or_many",
        serialize_with = "one_or_list"
    )]
    pub carvers: Vec<ResourceLocation<Arc<str>>>,
    #[serde(default)]
    pub features: Vec<FeatureStepList>,
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
        self.attributes.argument(id::NATURAL_MOB_SPAWNS.id.as_str())
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrassColorModifier {
    #[default]
    None,
    DarkForest,
    Swamp,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BiomeEffects {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_color: Option<HexRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foliage_color: Option<HexRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grass_color: Option<HexRgb>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub grass_color_modifier: GrassColorModifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_foliage_color: Option<HexRgb>,
}

pub type PlacedFeatureKey = ResourceKey<PlacedFeature, &'static str>;

/// The carvers and the per-step placed features of a biome, in the order they
/// were added. A step is listed up to the last one that was given a feature.
#[derive(Debug, Default)]
pub struct BiomeGeneration {
    carvers: Vec<StaticResourceLocation>,
    features: Vec<Vec<PlacedFeatureKey>>,
}

impl BiomeGeneration {
    pub fn carver(&mut self, key: ResourceKey<Carver, &'static str>) -> &mut Self {
        self.carvers.push(*key.location());
        self
    }

    pub fn feature(&mut self, step: DecorationStep, key: PlacedFeatureKey) -> &mut Self {
        self.features(step, &[key])
    }

    pub fn features(&mut self, step: DecorationStep, keys: &[PlacedFeatureKey]) -> &mut Self {
        let step = step as usize;
        if self.features.len() <= step {
            self.features.resize_with(step + 1, Vec::new);
        }
        self.features[step].extend_from_slice(keys);
        self
    }
}

impl Biome {
    pub const NORMAL_WATER_COLOR: i32 = 4159204;

    pub fn new(has_precipitation: bool, temperature: f32, downfall: f32) -> Self {
        Biome {
            temperature,
            downfall,
            has_precipitation,
            temperature_modifier: None,
            effects: BiomeEffects::default(),
            attributes: Default::default(),
            carvers: Vec::new(),
            features: Vec::new(),
        }
        .water(Self::NORMAL_WATER_COLOR)
    }

    pub fn spawns(self, mobs: MobSpawnSettings) -> Self {
        self.modified(id::NATURAL_MOB_SPAWNS, Operation::Overlay, mobs)
    }

    pub fn generation(mut self, generation: BiomeGeneration) -> Self {
        self.carvers = generation.carvers.into_iter().map(Into::into).collect();
        let steps = generation.features.into_iter();
        self.features = steps
            .map(|step| HolderSet::List(step.into_iter().map(Into::into).collect()))
            .collect();
        self
    }

    pub fn with<T: Serialize>(self, attribute: Attribute<T>, value: T) -> Self {
        self.modified(attribute, Operation::Override, value)
    }

    /// Panics on an argument the attribute or the modifier does not take: a
    /// description written in code is wrong at the point it is written.
    pub fn modified<T>(
        mut self,
        attribute: Attribute<T>,
        modifier: Operation,
        argument: impl Serialize,
    ) -> Self {
        if let Err(error) = self.attributes.modify(attribute.id, modifier, argument) {
            panic!("{}: {error}", attribute.id);
        }
        self
    }

    pub fn precipitation(mut self, has_precipitation: bool) -> Self {
        self.has_precipitation = has_precipitation;
        self
    }

    pub fn temperature_modifier(mut self, modifier: TemperatureModifier) -> Self {
        self.temperature_modifier = Some(modifier);
        self
    }

    pub fn water(mut self, color: i32) -> Self {
        self.effects.water_color = Some(HexRgb::of(color));
        self
    }

    pub fn foliage(mut self, color: i32) -> Self {
        self.effects.foliage_color = Some(HexRgb::of(color));
        self
    }

    pub fn grass(mut self, color: i32) -> Self {
        self.effects.grass_color = Some(HexRgb::of(color));
        self
    }

    pub fn dry_foliage(mut self, color: i32) -> Self {
        self.effects.dry_foliage_color = Some(HexRgb::of(color));
        self
    }

    pub fn grass_modifier(mut self, modifier: GrassColorModifier) -> Self {
        self.effects.grass_color_modifier = modifier;
        self
    }
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

fn one_or_list<S: Serializer, T: Serialize>(items: &[T], serializer: S) -> Result<S::Ok, S::Error> {
    match items {
        [item] => item.serialize(serializer),
        items => items.serialize(serializer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_all_biomes() {
        let biomes = mcrs_minecraft_worldgen_testing::registry::<Biome>("biome");
        assert!(biomes.len() >= 78, "{} biomes", biomes.len());
        for (id, biome) in biomes {
            if let Err(e) = biome.natural_mob_spawns() {
                panic!("{id}: {e}");
            }
            let encoded = serde_json::to_string(&biome).unwrap();
            let read: Biome = serde_json::from_str(&encoded).unwrap();
            assert_eq!(read, biome, "{id} must round-trip unchanged");
        }
    }
}
