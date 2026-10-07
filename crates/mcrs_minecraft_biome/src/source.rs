use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::climate::{ClimateParameters, ParameterPoint};
use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_registry::{HolderSet, Id};

// ===========================================================================
// Beta biome lookup — enum, cascade, table
// ===========================================================================

/// Discriminant order is the contract for the `biomes` list order in the
/// `mcrs:beta` biome_source JSON. The JSON entry at index N must correspond to
/// the BetaLandBiome variant whose discriminant equals N.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BetaLandBiome {
    IceDesert = 0,
    Tundra = 1,
    Savanna = 2,
    Desert = 3,
    Swampland = 4,
    Taiga = 5,
    Shrubland = 6,
    Forest = 7,
    Plains = 8,
    SeasonalForest = 9,
    Rainforest = 10,
}

pub fn beta_get_biome(temp: f32, rain: f32) -> BetaLandBiome {
    let rain = rain * temp;
    if temp < 0.1 {
        return BetaLandBiome::IceDesert;
    }
    if rain < 0.2 {
        if temp < 0.5 {
            return BetaLandBiome::Tundra;
        }
        if temp < 0.95 {
            return BetaLandBiome::Savanna;
        }
        return BetaLandBiome::Desert;
    }
    if rain > 0.5 && temp < 0.7 {
        return BetaLandBiome::Swampland;
    }
    if temp < 0.5 {
        return BetaLandBiome::Taiga;
    }
    if temp < 0.97 {
        if rain < 0.35 {
            return BetaLandBiome::Shrubland;
        }
        return BetaLandBiome::Forest;
    }
    if rain < 0.45 {
        return BetaLandBiome::Plains;
    }
    if rain < 0.9 {
        return BetaLandBiome::SeasonalForest;
    }
    BetaLandBiome::Rainforest
}

pub fn build_beta_lookup_table() -> [[BetaLandBiome; 64]; 64] {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| beta_get_biome(i as f32 / 63.0, j as f32 / 63.0))
    })
}

/// Resolve a land biome via the precomputed 64x64 quantized lookup, mirroring
/// BiomeBase.getBiomeFromLookup: i=(int)(temp*63), j=(int)(rain*63).
pub fn beta_biome_from_climate(
    table: &[[BetaLandBiome; 64]; 64],
    temp: f32,
    rain: f32,
) -> BetaLandBiome {
    let ti = ((temp * 63.0) as usize).min(63);
    let ri = ((rain * 63.0) as usize).min(63);
    table[ti][ri]
}

// ===========================================================================
// Biome sources
// ===========================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum BiomeSource {
    MultiNoise(MultiNoiseBiomeSource),
    TheEnd,
    Fixed {
        biome: Id<crate::Biome>,
    },
    Checkerboard {
        biomes: HolderSet<crate::Biome>,
        #[serde(default, skip_serializing_if = "is_default")]
        scale: Bounded<0, 62, 2>,
    },
    Beta {
        // Indexed by BetaLandBiome discriminant (0..=10); the JSON biomes list
        // order must match those discriminant values.
        #[serde(rename = "biomes")]
        land_biomes: [Id<crate::Biome>; 11],
        #[serde(skip, default = "beta_lookup")]
        lookup: Arc<[[BetaLandBiome; 64]; 64]>,
    },
}

mcrs_minecraft_registry::dispatch! {
    BiomeSource, key = "type", registry = crate::keys::BiomeSourceType,
    {
        MultiNoise => MultiNoise,
        TheEnd => TheEnd,
        Fixed => Fixed,
        Checkerboard => Checkerboard,
    }
    extend { "mcrs:beta" => Beta }
}

fn beta_lookup() -> Arc<[[BetaLandBiome; 64]; 64]> {
    Arc::new(build_beta_lookup_table())
}

impl BiomeSource {
    pub fn beta_biome(&self, temp: f32, rain: f32) -> Id<crate::Biome> {
        match self {
            BiomeSource::Beta {
                land_biomes,
                lookup,
            } => land_biomes[beta_biome_from_climate(lookup, temp, rain) as usize],
            _ => panic!("beta_biome called on non-Beta BiomeSource"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MultiNoiseBiomeSource {
    Biomes(Vec<MultiNoiseBiomeEntry>),
    Preset(Id<crate::parameter_list::MultiNoiseBiomeSourceParameterList>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MultiNoiseFields {
    #[serde(default, deserialize_with = "distinguishable_entries")]
    biomes: Option<Vec<MultiNoiseBiomeEntry>>,
    #[serde(default)]
    preset: Option<Id<crate::parameter_list::MultiNoiseBiomeSourceParameterList>>,
}

impl<'de> Deserialize<'de> for MultiNoiseBiomeSource {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match MultiNoiseFields::deserialize(d)? {
            MultiNoiseFields {
                biomes: Some(biomes),
                preset: None,
            } => Ok(Self::Biomes(biomes)),
            MultiNoiseFields {
                biomes: None,
                preset: Some(preset),
            } => Ok(Self::Preset(preset)),
            MultiNoiseFields { biomes: None, .. } => Err(serde::de::Error::custom(
                "a multi-noise biome source needs `biomes` or `preset`",
            )),
            MultiNoiseFields { .. } => Err(serde::de::Error::custom(
                "a multi-noise biome source takes `biomes` or `preset`, not both",
            )),
        }
    }
}

impl Serialize for MultiNoiseBiomeSource {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut fields = s.serialize_struct("MultiNoiseBiomeSource", 1)?;
        match self {
            Self::Biomes(biomes) => fields.serialize_field("biomes", biomes)?,
            Self::Preset(preset) => fields.serialize_field("preset", preset)?,
        }
        fields.end()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiNoiseBiomeEntry {
    pub parameters: ClimateParameters,
    pub biome: Id<crate::Biome>,
}

fn distinguishable_entries<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<MultiNoiseBiomeEntry>>, D::Error> {
    let entries = Vec::<MultiNoiseBiomeEntry>::deserialize(deserializer)?;
    if entries.is_empty() {
        return Err(serde::de::Error::custom("List must have contents"));
    }
    let points: Vec<_> = entries
        .iter()
        .map(|entry| (entry.biome, ParameterPoint::from(&entry.parameters)))
        .collect();
    for (first, (biome_a, a)) in points.iter().enumerate() {
        for (second, (biome_b, b)) in points.iter().enumerate().skip(first + 1) {
            if biome_a != biome_b && a.indistinguishable_from(b) {
                return Err(serde::de::Error::custom(format!(
                    "Entries {first} and {second} overlap in all noise parameters"
                )));
            }
        }
    }
    Ok(Some(entries))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_registry::{Registry, RegistrySet};
    use std::collections::HashSet;
    use std::sync::Arc;

    #[test]
    fn two_biomes_no_climate_can_tell_apart_are_a_load_error() {
        let names = ["minecraft:plains", "minecraft:desert"]
            .map(|name| ResourceLocation::<Arc<str>>::read(name).unwrap());
        let set = RegistrySet::new()
            .with(Registry::<crate::Biome>::new(crate::keys::BIOME, names).unwrap())
            .unwrap();
        let entry = |biome: &str, humidity: &str| {
            format!(
                r#"{{"biome":"{biome}","parameters":{{"temperature":0.0,"humidity":{humidity},"continentalness":0.0,"erosion":0.0,"depth":0.0,"weirdness":0.0,"offset":0.0}}}}"#
            )
        };
        let parse = |a: String, b: String| {
            set.scope(|| {
                serde_json::from_str::<MultiNoiseBiomeSource>(&format!(r#"{{"biomes":[{a},{b}]}}"#))
            })
        };

        let error = parse(
            entry("minecraft:plains", "0.0"),
            entry("minecraft:desert", "0.0"),
        )
        .expect_err("two biomes on one climate point")
        .to_string();
        assert!(error.contains("Entries 0 and 1 overlap"), "{error}");
        assert!(
            parse(
                entry("minecraft:plains", "0.0"),
                entry("minecraft:plains", "0.0")
            )
            .is_ok()
        );
        assert!(
            parse(
                entry("minecraft:plains", "[-1.0, 0.7]"),
                entry("minecraft:desert", "[0.7, 1.0]")
            )
            .is_ok()
        );
    }

    fn source_set() -> RegistrySet {
        let read = |name: &str| ResourceLocation::<Arc<str>>::read(name).unwrap();
        let biomes = Registry::<crate::Biome>::new(
            crate::keys::BIOME,
            ["minecraft:plains", "minecraft:desert"].map(read),
        )
        .unwrap();
        let tag = r#"{"values":["minecraft:desert","minecraft:plains"]}"#;
        let files = [(
            read("minecraft:warm"),
            vec![mcrs_minecraft_registry::tags::TagSource {
                pack: "test",
                path: "minecraft/tags/worldgen/biome/warm.json",
                bytes: tag.as_bytes(),
            }],
        )];
        let (tags, problems) = mcrs_minecraft_registry::build_tags(
            biomes.table(),
            mcrs_minecraft_registry::TagRules::World,
            &files,
            None,
        );
        assert!(problems.is_empty(), "{problems:?}");
        RegistrySet::new()
            .with(biomes)
            .unwrap()
            .with(
                Registry::<crate::parameter_list::MultiNoiseBiomeSourceParameterList>::new(
                    crate::keys::MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                    [read("minecraft:overworld")],
                )
                .unwrap(),
            )
            .unwrap()
            .with_tags(Arc::new(tags))
    }

    #[test]
    fn a_biome_source_reads_and_writes_the_shapes_the_reference_does() {
        let entry = r#"{"parameters":{"temperature":0.0,"humidity":0.0,"continentalness":0.0,"erosion":0.0,"depth":0.0,"weirdness":0.0,"offset":0.0},"biome":"minecraft:plains"}"#;
        let round_trips = [
            r#"{"type":"minecraft:multi_noise","preset":"minecraft:overworld"}"#.to_owned(),
            format!(r#"{{"type":"minecraft:multi_noise","biomes":[{entry}]}}"#),
            r##"{"type":"minecraft:checkerboard","biomes":"#minecraft:warm"}"##.to_owned(),
            r#"{"type":"minecraft:checkerboard","biomes":["minecraft:plains","minecraft:desert"],"scale":62}"#.to_owned(),
            r#"{"type":"minecraft:checkerboard","biomes":"minecraft:plains"}"#.to_owned(),
        ];
        let set = source_set();
        set.scope(|| {
            for text in &round_trips {
                let read: BiomeSource =
                    serde_json::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
                assert_eq!(&serde_json::to_string(&read).unwrap(), text);
            }
            let explicit: BiomeSource = serde_json::from_str(
                r#"{"type":"minecraft:checkerboard","biomes":["minecraft:plains"],"scale":2}"#,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_string(&explicit).unwrap(),
                r#"{"type":"minecraft:checkerboard","biomes":"minecraft:plains"}"#
            );
        });
    }

    #[test]
    fn a_biome_source_the_reference_refuses_is_a_load_error() {
        let entry = r#"{"parameters":{"temperature":0.0,"humidity":0.0,"continentalness":0.0,"erosion":0.0,"depth":0.0,"weirdness":0.0,"offset":0.0},"biome":"minecraft:plains"}"#;
        let refused = [
            (
                r#"{"type":"minecraft:multi_noise"}"#.to_owned(),
                "needs `biomes` or `preset`",
            ),
            (
                format!(
                    r#"{{"type":"minecraft:multi_noise","preset":"minecraft:overworld","biomes":[{entry}]}}"#
                ),
                "not both",
            ),
            (
                r#"{"type":"minecraft:multi_noise","biomes":[]}"#.to_owned(),
                "List must have contents",
            ),
            (
                r#"{"type":"minecraft:checkerboard","biomes":["minecraft:plains"],"scale":63}"#
                    .to_owned(),
                "Value must be within range [0;62]: 63",
            ),
            (
                r#"{"type":"minecraft:checkerboard","biomes":["minecraft:plains"],"scale":-1}"#
                    .to_owned(),
                "Value must be within range [0;62]: -1",
            ),
        ];
        let set = source_set();
        set.scope(|| {
            for (text, expected) in &refused {
                let error = serde_json::from_str::<BiomeSource>(text)
                    .expect_err(text)
                    .to_string();
                assert!(error.contains(expected), "{text}: {error}");
            }
        });
    }

    #[test]
    fn beta_biome_all_land_reachable() {
        let table = build_beta_lookup_table();
        let mut seen: HashSet<u8> = HashSet::new();
        for row in &table {
            for biome in row {
                seen.insert(*biome as u8);
            }
        }
        assert_eq!(
            seen.len(),
            11,
            "Expected all 11 land buckets to be reachable, got {:?}",
            seen
        );
        // Verify each specific bucket is present
        for expected in 0u8..=10 {
            assert!(
                seen.contains(&expected),
                "Bucket with discriminant {} is not reachable from the 64x64 table",
                expected
            );
        }
    }

    #[test]
    fn beta_biome_lookup_table() {
        // temp < 0.1 → IceDesert
        assert_eq!(beta_get_biome(0.05, 0.5), BetaLandBiome::IceDesert);

        // temp=0.97, rain_adjusted = rain*temp; test Savanna: temp in [0.5,0.95), rain*temp < 0.2
        // temp=0.6, rain=0.1 → rain*temp=0.06 < 0.2, temp >= 0.5, temp < 0.95 → Savanna
        assert_eq!(beta_get_biome(0.6, 0.1), BetaLandBiome::Savanna);

        // Desert: temp >= 0.95, rain*temp < 0.2
        // temp=0.96, rain=0.1 → rain*temp=0.096 < 0.2, temp >= 0.95 → Desert
        assert_eq!(beta_get_biome(0.96, 0.1), BetaLandBiome::Desert);

        // SeasonalForest: temp >= 0.97, rain*temp in [0.45, 0.9)
        // temp=0.98, rain=0.5 → rain*temp=0.49 ≥ 0.45 and < 0.9 → SeasonalForest
        assert_eq!(beta_get_biome(0.98, 0.5), BetaLandBiome::SeasonalForest);

        // Rainforest: temp >= 0.97, rain*temp >= 0.9
        // temp=0.98, rain=0.95 → rain*temp=0.931 ≥ 0.9 → Rainforest
        assert_eq!(beta_get_biome(0.98, 0.95), BetaLandBiome::Rainforest);

        // Tundra: temp < 0.5, rain*temp < 0.2 (and temp >= 0.1)
        // temp=0.3, rain=0.5 → rain*temp=0.15 < 0.2, temp < 0.5 → Tundra
        assert_eq!(beta_get_biome(0.3, 0.5), BetaLandBiome::Tundra);

        // Swampland: rain*temp > 0.5, temp < 0.7
        // temp=0.6, rain=0.9 → rain*temp=0.54 > 0.5, temp < 0.7 → Swampland
        assert_eq!(beta_get_biome(0.6, 0.9), BetaLandBiome::Swampland);

        // Taiga: temp >= 0.5, rain*temp >= 0.2, rain*temp <= 0.5 (or temp < 0.7), temp < 0.5 fails → need temp in [0.5, 0.7) and rain not swampland
        // Actually Taiga: temp < 0.5 — wait let's re-check: after swampland check, if temp < 0.5 → Taiga
        // temp=0.4, rain=0.9 → rain*temp=0.36 >= 0.2, rain*temp <= 0.5 (0.36 not > 0.5), temp < 0.5 → Taiga
        assert_eq!(beta_get_biome(0.4, 0.9), BetaLandBiome::Taiga);

        // Shrubland: temp >= 0.5, temp < 0.97, rain*temp < 0.35
        // temp=0.7, rain=0.4 → rain*temp=0.28 < 0.35, temp < 0.97 → Shrubland
        assert_eq!(beta_get_biome(0.7, 0.4), BetaLandBiome::Shrubland);

        // Forest: temp >= 0.5, temp < 0.97, rain*temp >= 0.35
        // temp=0.7, rain=0.6 → rain*temp=0.42 >= 0.35, rain*temp <= 0.5 (not swampland since 0.42 ≤ 0.5), temp < 0.97 → Forest
        assert_eq!(beta_get_biome(0.7, 0.6), BetaLandBiome::Forest);

        // Plains: temp >= 0.97, rain*temp < 0.45
        // temp=0.98, rain=0.4 → rain*temp=0.392 < 0.45 → Plains
        assert_eq!(beta_get_biome(0.98, 0.4), BetaLandBiome::Plains);

        // Confirm rain is multiplied by temp before comparisons:
        // temp=0.3, rain=0.8 → rain*temp=0.24 >= 0.2, rain*temp <= 0.5 (no swampland), temp < 0.5 → Taiga
        // Without multiplication: rain=0.8 > 0.5 and temp=0.3 < 0.7 → Swampland (wrong)
        assert_eq!(beta_get_biome(0.3, 0.8), BetaLandBiome::Taiga);
    }
}
