use serde::{Deserialize, Serialize, Serializer};

use mcrs_minecraft_biome::{
    BiomeEffects, ConfiguredCarver, DecorationStep, GrassColorModifier, PlacedFeature,
    TemperatureModifier,
};
use mcrs_minecraft_core::codec::HexRgb;
use mcrs_minecraft_environment::attribute::id::{self, Attribute};
use mcrs_minecraft_environment::attribute::{
    AttributeError, EnvironmentAttributeMap, MobSpawnSettings, Operation,
};
use mcrs_minecraft_registries::{KeySet, Named, Refs, Resolve, Resolved};
use mcrs_minecraft_resource::{ResourceKey, ResourceKeyBuf, ResourceLocationBuf};

pub type Carvers = ConfiguredCarver;
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

impl mcrs_minecraft_resource::RegistryValue for BiomeFile {
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
///
/// The builder methods chain, so the first argument an attribute refuses is
/// kept and [`BiomeDraft::finish`] returns it.
#[derive(Debug)]
pub struct BiomeDraft {
    biome: BiomeFile<Named>,
    refused: Option<AttributeError>,
}

/// Biome data subset for NETWORK_CODEC — omits server-only generation settings.
///
/// Sent to clients during Configuration; excludes carvers, features, and the
/// attributes the client is not allowed to see.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkBiome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_modifier: Option<TemperatureModifier>,
    #[serde(deserialize_with = "syncable")]
    pub attributes: EnvironmentAttributeMap,
    pub effects: BiomeEffects,
}

fn syncable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<EnvironmentAttributeMap, D::Error> {
    EnvironmentAttributeMap::deserialize(d).map(|attributes| attributes.filter_syncable())
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

pub type PlacedFeatureKey = ResourceKey<'static, PlacedFeature>;

/// The carvers and the per-step placed features of a biome, in the order they
/// were added. A step is listed up to the last one that was given a feature.
#[derive(Debug, Default)]
pub struct BiomeGeneration {
    carvers: Vec<ResourceKeyBuf<Carvers>>,
    features: Vec<Vec<ResourceKeyBuf<PlacedFeatures>>>,
}

fn key<R, V>(key: ResourceKey<'static, V>) -> ResourceKeyBuf<R> {
    ResourceKeyBuf::from_location(ResourceLocationBuf::from(key.location()))
}

impl BiomeGeneration {
    pub fn carver(&mut self, carver: ResourceKey<'static, ConfiguredCarver>) -> &mut Self {
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
        if let Some(features) = self.features.get_mut(step) {
            features.extend(keys.iter().copied().map(key));
        }
        self
    }
}

impl BiomeDraft {
    pub fn generation(mut self, generation: BiomeGeneration) -> Self {
        self.biome.carvers = KeySet::List(generation.carvers.into());
        self.biome.features = generation
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
        BiomeDraft {
            biome: BiomeFile {
                temperature,
                downfall,
                has_precipitation,
                temperature_modifier: None,
                effects: BiomeEffects::default(),
                attributes: Default::default(),
                carvers: KeySet::default(),
                features: Vec::new(),
            },
            refused: None,
        }
        .water(Self::NORMAL_WATER_COLOR)
    }

    /// The biome file, or the first attribute argument the draft was given that
    /// the attribute or its modifier does not take.
    pub fn finish(self) -> Result<BiomeFile<Named>, AttributeError> {
        match self.refused {
            Some(error) => Err(error),
            None => Ok(self.biome),
        }
    }

    pub fn spawns(self, mobs: MobSpawnSettings) -> Self {
        self.modified(id::NATURAL_MOB_SPAWNS, Operation::Overlay, mobs)
    }

    pub fn with<T: Serialize>(self, attribute: Attribute<T>, value: T) -> Self {
        self.modified(attribute, Operation::Override, value)
    }

    /// Leaves the attribute unchanged on an argument the attribute or the
    /// modifier does not take, and [`BiomeDraft::finish`] reports it.
    pub fn modified<T>(
        mut self,
        attribute: Attribute<T>,
        modifier: Operation,
        argument: impl Serialize,
    ) -> Self {
        if self.refused.is_none()
            && let Err(error) = self
                .biome
                .attributes
                .modify(attribute.id, modifier, argument)
        {
            self.refused = Some(error);
        }
        self
    }

    pub fn precipitation(mut self, has_precipitation: bool) -> Self {
        self.biome.has_precipitation = has_precipitation;
        self
    }

    pub fn temperature_modifier(mut self, modifier: TemperatureModifier) -> Self {
        self.biome.temperature_modifier = Some(modifier);
        self
    }

    pub fn water(mut self, color: i32) -> Self {
        self.biome.effects.water_color = Some(HexRgb::of(color));
        self
    }

    pub fn foliage(mut self, color: i32) -> Self {
        self.biome.effects.foliage_color = Some(HexRgb::of(color));
        self
    }

    pub fn grass(mut self, color: i32) -> Self {
        self.biome.effects.grass_color = Some(HexRgb::of(color));
        self
    }

    pub fn dry_foliage(mut self, color: i32) -> Self {
        self.biome.effects.dry_foliage_color = Some(HexRgb::of(color));
        self
    }

    pub fn grass_modifier(mut self, modifier: GrassColorModifier) -> Self {
        self.biome.effects.grass_color_modifier = modifier;
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
