pub mod beta_surface;
pub mod climate;
pub mod overworld_preset;
pub mod parameter_list;
pub mod source;
pub mod zoom;

use serde::{Deserialize, Serialize};

use mcrs_minecraft_core::codec::{HexRgb, is_default};
use mcrs_minecraft_core::{RegistryKey, ResourceKey, StaticResourceLocation};
use mcrs_minecraft_environment::attribute::id::{self, Attribute};
use mcrs_minecraft_environment::attribute::{
    AttributeValue, EnvironmentAttributeMap, MobSpawnSettings, Operation,
};
use mcrs_minecraft_keys::{Carver, PlacedFeature};
use mcrs_minecraft_registry::{HolderSet, Id, RegistrySet};
use mcrs_minecraft_worldgen_structure::DecorationStep;

pub use mcrs_minecraft_worldgen_structure::{MobCategory, SpawnerData};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureModifier {
    None,
    Frozen,
}

pub type CarverSet = HolderSet<Carver>;
pub type FeatureSteps = Vec<HolderSet<PlacedFeature>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Biome<C = CarverSet, F = FeatureSteps> {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<TemperatureModifier>,
    pub effects: BiomeEffects,
    #[serde(default, skip_serializing_if = "EnvironmentAttributeMap::is_empty")]
    pub attributes: EnvironmentAttributeMap,
    #[serde(default)]
    pub carvers: C,
    #[serde(default)]
    pub features: F,
}

/// A biome described in code, before any registry has ids: carvers and
/// placed features are still keys.
pub type BiomeDraft = Biome<Vec<StaticResourceLocation>, Vec<Vec<PlacedFeatureKey>>>;

impl<C, F> Biome<C, F> {
    pub fn natural_mob_spawns(&self) -> Option<&MobSpawnSettings> {
        match self
            .attributes
            .argument(id::NATURAL_MOB_SPAWNS.id.as_str())?
        {
            AttributeValue::MobSpawns(spawns) => Some(spawns),
            _ => None,
        }
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

fn ids<R: RegistryKey>(
    set: &RegistrySet,
    names: &[StaticResourceLocation],
    failures: &mut Vec<String>,
) -> Vec<Id<R>> {
    let Some(registry) = set.registry::<R>() else {
        if !names.is_empty() {
            failures.push(format!("the registry {} is not loaded", R::KEY));
        }
        return Vec::new();
    };
    names
        .iter()
        .filter_map(|name| {
            let id = registry.get(name.as_str());
            if id.is_none() {
                failures.push(format!("{name} is not an entry of {}", R::KEY));
            }
            id
        })
        .collect()
}

impl BiomeDraft {
    pub fn generation(mut self, generation: BiomeGeneration) -> Self {
        self.carvers = generation.carvers;
        self.features = generation.features;
        self
    }

    /// Every key that names no entry of its registry is reported, so a
    /// biome that cannot be built says everything wrong with it at once.
    pub fn resolve(self, set: &RegistrySet) -> Result<Biome, Vec<String>> {
        let mut failures = Vec::new();
        let carvers = match ids::<Carver>(set, &self.carvers, &mut failures)[..] {
            [only] => HolderSet::One(only),
            ref listed => HolderSet::List(listed.into()),
        };
        let features = self
            .features
            .iter()
            .map(|step| {
                let names: Vec<_> = step.iter().map(|key| *key.location()).collect();
                HolderSet::List(ids::<PlacedFeature>(set, &names, &mut failures).into())
            })
            .collect();
        if !failures.is_empty() {
            return Err(failures);
        }
        Ok(Biome {
            temperature: self.temperature,
            downfall: self.downfall,
            has_precipitation: self.has_precipitation,
            temperature_modifier: self.temperature_modifier,
            effects: self.effects,
            attributes: self.attributes,
            carvers,
            features,
        })
    }
}

impl<C: Default, F: Default> Biome<C, F> {
    pub const NORMAL_WATER_COLOR: i32 = 4159204;

    pub fn new(has_precipitation: bool, temperature: f32, downfall: f32) -> Self {
        Biome {
            temperature,
            downfall,
            has_precipitation,
            temperature_modifier: None,
            effects: BiomeEffects::default(),
            attributes: Default::default(),
            carvers: C::default(),
            features: F::default(),
        }
        .water(Self::NORMAL_WATER_COLOR)
    }

    pub fn spawns(self, mobs: MobSpawnSettings) -> Self {
        self.modified(id::NATURAL_MOB_SPAWNS, Operation::Overlay, mobs)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_all_biomes() {
        let biomes = mcrs_minecraft_worldgen_testing::registry::<Biome>("biome");
        assert!(biomes.len() >= 78, "{} biomes", biomes.len());
        for (id, biome) in biomes {
            mcrs_minecraft_worldgen_testing::corpus_set().scope(|| {
                let encoded = serde_json::to_string(&biome).unwrap();
                let read: Biome = serde_json::from_str(&encoded).unwrap();
                assert_eq!(read, biome, "{id} must round-trip unchanged");
            });
        }
    }
}
