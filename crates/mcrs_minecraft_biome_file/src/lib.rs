use serde::{Deserialize, Serialize, Serializer};

use mcrs_minecraft_biome::{BiomeEffects, GrassColorModifier, TemperatureModifier};
use mcrs_minecraft_core::codec::HexRgb;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_environment::attribute::id::{self, Attribute};
use mcrs_minecraft_environment::attribute::{EnvironmentAttributeMap, MobSpawnSettings, Operation};
use mcrs_minecraft_registries::{
    DataDeclaration, Declaration, KeySet, Named, Refs, Resolve, Resolved,
};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_feature::placement::DecorationStep;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;

mod validate;

pub use validate::check_feature_domains;

pub type Carvers = CarverConfig;
pub type PlacedFeatures = PlacedFeature;

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

impl mcrs_minecraft_core::RegistryValue for BiomeFile {
    type Registry = mcrs_minecraft_biome::Biome;
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

mcrs_minecraft_registries::resolve_as_is!(NetworkBiome);

impl From<&BiomeFile> for NetworkBiome {
    fn from(biome: &BiomeFile) -> Self {
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

/// A client holds no generation settings: the network leaves them out.
impl From<NetworkBiome> for BiomeFile {
    fn from(biome: NetworkBiome) -> Self {
        BiomeFile {
            temperature: biome.temperature,
            downfall: biome.downfall,
            has_precipitation: biome.has_precipitation,
            temperature_modifier: biome.temperature_modifier,
            effects: biome.effects,
            attributes: biome.attributes,
            carvers: Default::default(),
            features: Vec::new(),
        }
    }
}

pub type PlacedFeatureKey = ResourceKey<PlacedFeature, &'static str>;

/// The carvers and the per-step placed features of a biome, in the order they
/// were added. A step is listed up to the last one that was given a feature.
#[derive(Debug, Default)]
pub struct BiomeGeneration {
    carvers: Vec<ResourceKey<Carvers>>,
    features: Vec<Vec<ResourceKey<PlacedFeatures>>>,
}

fn key<R, V>(key: ResourceKey<V, &'static str>) -> ResourceKey<R> {
    ResourceKey::from_location(ResourceLocation::from(*key.location()))
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
    pub fn declaration() -> DataDeclaration<Self> {
        Declaration::data::<Self>().validate(|registry, ctx| {
            let (Some(placed), Some(tags)) =
                (ctx.registry::<PlacedFeatures>(), ctx.tags::<PlacedFeatures>())
            else {
                return Vec::new();
            };
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
            check_feature_domains(biomes)
        })
    }
}
