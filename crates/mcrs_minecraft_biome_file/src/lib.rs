use serde::{Deserialize, Serialize, Serializer};

use mcrs_minecraft_biome::{Biome, BiomeEffects, GrassColorModifier, TemperatureModifier};
use mcrs_minecraft_core::codec::HexRgb;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_environment::attribute::id::{self, Attribute};
use mcrs_minecraft_environment::attribute::{EnvironmentAttributeMap, MobSpawnSettings, Operation};
use mcrs_minecraft_registries::{
    DataDeclaration, Declaration, Key, KeySet, Named, Refs, RegistryOf, Resolve, Resolved, Stage,
};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_feature::placement::DecorationStep;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;

mod validate;

pub use validate::check_feature_domains;

pub type Carvers = RegistryOf<CarverConfig>;
pub type PlacedFeatures = RegistryOf<PlacedFeature>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Resolve)]
#[serde(bound(
    serialize = "M::Set<Carvers>: Serialize, M::Set<PlacedFeatures>: ListForm",
    deserialize = "M::Set<Carvers>: Deserialize<'de> + Default, M::Set<PlacedFeatures>: Deserialize<'de>"
))]
pub struct BiomeFile<M: Refs = Resolved> {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<TemperatureModifier>,
    pub effects: BiomeEffects,
    #[serde(default, skip_serializing_if = "EnvironmentAttributeMap::is_empty")]
    pub attributes: EnvironmentAttributeMap,
    #[serde(default)]
    pub carvers: M::Set<Carvers>,
    #[serde(default, serialize_with = "serialize_steps")]
    pub features: Vec<M::Set<PlacedFeatures>>,
}

/// A decoration step writes a single entry as a list, where any other set writes it bare.
pub trait ListForm {
    fn serialize_listed<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;
}

impl<R> ListForm for KeySet<R> {
    fn serialize_listed<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            KeySet::Tag(_) => self.serialize(s),
            KeySet::List(keys) => keys.serialize(s),
        }
    }
}

struct Listed<'a, T>(&'a T);

impl<T: ListForm> Serialize for Listed<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_listed(s)
    }
}

fn serialize_steps<T: ListForm, S: Serializer>(steps: &[T], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(steps.iter().map(Listed))
}

/// A biome described in code: carvers and placed features are still names.
pub type BiomeDraft = BiomeFile<Named>;

/// Biome data subset for NETWORK_CODEC — omits server-only generation settings.
///
/// Sent to clients during Configuration; excludes carvers, features, and the
/// attributes the client is not allowed to see.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkBiome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<TemperatureModifier>,
    pub attributes: EnvironmentAttributeMap,
    pub effects: BiomeEffects,
}

impl NetworkBiome {
    /// The parts a client holds of a biome: the generation settings the network leaves out are
    /// empty.
    pub fn into_client_parts(self) -> (Biome, EnvironmentAttributeMap, BiomeGenerationSettings) {
        let (biome, attributes) = self.into_parts();
        let generation = BiomeGenerationSettings {
            carvers: Default::default(),
            features: Vec::new(),
        };
        (biome, attributes, generation)
    }

    pub fn into_parts(self) -> (Biome, EnvironmentAttributeMap) {
        let biome = Biome {
            temperature: self.temperature,
            downfall: self.downfall,
            has_precipitation: self.has_precipitation,
            temperature_modifier: self.temperature_modifier,
            effects: self.effects,
        };
        (biome, self.attributes)
    }
}

impl From<(&Biome, &EnvironmentAttributeMap, &BiomeGenerationSettings)> for NetworkBiome {
    fn from(
        (biome, attributes, _): (&Biome, &EnvironmentAttributeMap, &BiomeGenerationSettings),
    ) -> Self {
        NetworkBiome {
            temperature: biome.temperature,
            downfall: biome.downfall,
            has_precipitation: biome.has_precipitation,
            temperature_modifier: biome.temperature_modifier,
            attributes: attributes.filter_syncable(),
            effects: biome.effects.clone(),
        }
    }
}

pub type PlacedFeatureKey = ResourceKey<PlacedFeature, &'static str>;

/// The carvers and the per-step placed features of a biome, in the order they
/// were added. A step is listed up to the last one that was given a feature.
#[derive(Debug, Default)]
pub struct BiomeGeneration {
    carvers: Vec<Key<Carvers>>,
    features: Vec<Vec<Key<PlacedFeatures>>>,
}

fn key<R, V>(key: ResourceKey<V, &'static str>) -> Key<R> {
    Key::new(ResourceLocation::from(*key.location()))
}

impl BiomeGeneration {
    pub fn carver(&mut self, carver: ResourceKey<CarverConfig, &'static str>) -> &mut Self {
        self.carvers.push(key(carver));
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
        self.features[step].extend(keys.iter().copied().map(key));
        self
    }
}

impl BiomeDraft {
    pub fn generation(mut self, generation: BiomeGeneration) -> Self {
        self.carvers = KeySet::List(generation.carvers.into());
        self.features = generation
            .features
            .into_iter()
            .map(|step| KeySet::List(step.into()))
            .collect();
        self
    }
}

impl BiomeDraft {
    pub const NORMAL_WATER_COLOR: i32 = 4159204;

    pub fn new(has_precipitation: bool, temperature: f32, downfall: f32) -> Self {
        BiomeFile {
            temperature,
            downfall,
            has_precipitation,
            temperature_modifier: None,
            effects: BiomeEffects::default(),
            attributes: Default::default(),
            carvers: KeySet::default(),
            features: Vec::new(),
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

/// The carvers and the per-step placed features of a biome, the column the
/// generator reads.
#[derive(Debug, Clone, PartialEq)]
pub struct BiomeGenerationSettings {
    pub carvers: mcrs_minecraft_registries::IdSet<Carvers>,
    pub features: Vec<mcrs_minecraft_registries::IdSet<PlacedFeatures>>,
}

impl BiomeFile {
    pub fn split(&self) -> (Biome, EnvironmentAttributeMap, BiomeGenerationSettings) {
        let biome = Biome {
            temperature: self.temperature,
            downfall: self.downfall,
            has_precipitation: self.has_precipitation,
            temperature_modifier: self.temperature_modifier,
            effects: self.effects.clone(),
        };
        let generation = BiomeGenerationSettings {
            carvers: self.carvers.clone(),
            features: self.features.clone(),
        };
        (biome, self.attributes.clone(), generation)
    }

    pub fn join(
        (biome, attributes, generation): (
            &Biome,
            &EnvironmentAttributeMap,
            &BiomeGenerationSettings,
        ),
    ) -> Self {
        BiomeFile {
            temperature: biome.temperature,
            downfall: biome.downfall,
            has_precipitation: biome.has_precipitation,
            temperature_modifier: biome.temperature_modifier,
            effects: biome.effects.clone(),
            attributes: attributes.clone(),
            carvers: generation.carvers.clone(),
            features: generation.features.clone(),
        }
    }
}

impl BiomeFile {
    pub fn declaration() -> DataDeclaration<RegistryOf<Self>> {
        let key = mcrs_minecraft_biome::keys::BIOME.location().into();
        Declaration::data::<RegistryOf<Self>>(key, Stage::World)
            .named_by::<mcrs_minecraft_biome::Biome>()
            .validate(|registry, ctx| {
                let placed = ctx
                    .registry::<PlacedFeatures>()
                    .ok_or_else(|| "the placed features are not loaded".to_owned())?;
                let tags = ctx
                    .tags::<PlacedFeatures>()
                    .ok_or_else(|| "the placed feature tags are not loaded".to_owned())?;
                let biomes = registry.values().iter().map(|biome| {
                    biome
                        .features
                        .iter()
                        .map(|step| {
                            step.ids(&tags)
                                .filter_map(|id| Some((placed.name(id)?, placed.get(id)?)))
                                .collect()
                        })
                        .collect()
                });
                registry.entry_failures(check_feature_domains(biomes))
            })
    }
}
