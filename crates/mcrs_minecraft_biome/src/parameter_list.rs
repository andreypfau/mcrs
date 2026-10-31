use mcrs_minecraft_resource::ResourceLocation;
use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::climate::ParameterList;
use crate::overworld_preset::{nether_parameter_list, overworld_parameter_list};
use mcrs_minecraft_registries::Registries;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Overworld,
    Nether,
}

impl Preset {
    pub const ALL: [Preset; 2] = [Preset::Overworld, Preset::Nether];

    pub fn name(self) -> ResourceLocation<'static> {
        match self {
            Preset::Overworld => {
                crate::keys::multi_noise_biome_source_parameter_list::OVERWORLD.location()
            }
            Preset::Nether => {
                crate::keys::multi_noise_biome_source_parameter_list::NETHER.location()
            }
        }
    }

    pub fn parameter_list(self) -> &'static ParameterList<ResourceLocation<'static>> {
        match self {
            Preset::Overworld => overworld_parameter_list(),
            Preset::Nether => nether_parameter_list(),
        }
    }
}

impl Serialize for Preset {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.name().serialize(serializer)
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
                    .find(|preset| preset.name() == name)
                    .ok_or_else(|| E::custom(format_args!("Unknown preset: {name}")))
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

pub type ParameterLists = Arc<[MultiNoiseBiomeSourceParameterList]>;

pub fn parameter_lists_of(registries: &Registries) -> Option<ParameterLists> {
    registries
        .registry::<MultiNoiseBiomeSourceParameterList>()
        .map(|registry| Arc::clone(registry.values()))
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
