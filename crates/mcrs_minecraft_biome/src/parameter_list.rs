use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::climate::ParameterList;
use crate::overworld_preset::{nether_parameter_list, overworld_parameter_list};
use mcrs_minecraft_core::{RegistryKey, ResourceLocation};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Entries, RegistrySet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Overworld,
    Nether,
}

impl Preset {
    pub const ALL: [Preset; 2] = [Preset::Overworld, Preset::Nether];

    pub fn name(self) -> &'static str {
        match self {
            Preset::Overworld => keys::multi_noise_biome_source_parameter_list::OVERWORLD.as_str(),
            Preset::Nether => keys::multi_noise_biome_source_parameter_list::NETHER.as_str(),
        }
    }

    pub fn parameter_list(self) -> &'static ParameterList<&'static str> {
        match self {
            Preset::Overworld => overworld_parameter_list(),
            Preset::Nether => nether_parameter_list(),
        }
    }
}

impl Serialize for Preset {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Preset {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PresetVisitor;

        impl Visitor<'_> for PresetVisitor {
            type Value = Preset;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("the name of a built-in multi-noise parameter list preset")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Preset, E> {
                let name = ResourceLocation::read(text).map_err(E::custom)?;
                Preset::ALL
                    .into_iter()
                    .find(|preset| preset.name() == name.as_str())
                    .ok_or_else(|| E::custom(format_args!("Unknown preset: {}", name.as_str())))
            }
        }

        deserializer.deserialize_str(PresetVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiNoiseBiomeSourceParameterList {
    pub preset: Preset,
}

pub type ParameterLists =
    Entries<keys::MultiNoiseBiomeSourceParameterList, MultiNoiseBiomeSourceParameterList>;

pub fn parameter_lists_of(set: &RegistrySet) -> ParameterLists {
    set.entries().expect(
        "the data pack loader parses minecraft:worldgen/multi_noise_biome_source_parameter_list",
    )
}

pub fn check_parameter_list_biomes(
    lists: &[MultiNoiseBiomeSourceParameterList],
    set: &RegistrySet,
) -> Vec<(usize, String)> {
    let Some(biomes) = set.registry::<keys::Biome>() else {
        return Vec::new();
    };
    let mut failures = Vec::new();
    for (index, list) in lists.iter().enumerate() {
        let mut reported: Vec<&str> = Vec::new();
        for (_, biome) in list.preset.parameter_list().values() {
            if biomes.get(biome).is_none() && !reported.contains(biome) {
                reported.push(biome);
                failures.push((
                    index,
                    format!(
                        "the preset {} names the biome {biome}, which {} does not hold",
                        list.preset.name(),
                        keys::Biome::KEY,
                    ),
                ));
            }
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parameter_list_preset_reads_as_vanilla_reads_it() {
        let cases: [(&str, Result<Preset, &str>); 7] = [
            ("\"minecraft:overworld\"", Ok(Preset::Overworld)),
            ("\"overworld\"", Ok(Preset::Overworld)),
            ("\":nether\"", Ok(Preset::Nether)),
            ("\"minecraft:end\"", Err("Unknown preset: minecraft:end")),
            ("\"end\"", Err("Unknown preset: minecraft:end")),
            (
                "\"Minecraft:overworld\"",
                Err("Non [a-z0-9_.-] character in namespace of identifier"),
            ),
            ("7", Err("invalid type")),
        ];
        for (text, expected) in cases {
            let read = serde_json::from_str::<Preset>(text);
            match expected {
                Ok(preset) => assert_eq!(read.unwrap(), preset, "{text}"),
                Err(message) => {
                    let error = read.expect_err(text).to_string();
                    assert!(error.contains(message), "{text}: {error}");
                }
            }
        }

        let extra = r#"{"preset":"minecraft:overworld","extra":1}"#;
        let error = serde_json::from_str::<MultiNoiseBiomeSourceParameterList>(extra)
            .expect_err("an extra field is refused")
            .to_string();
        assert!(error.contains("unknown field `extra`"), "{error}");
    }
}
