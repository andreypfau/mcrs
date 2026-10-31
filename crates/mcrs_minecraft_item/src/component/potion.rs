use mcrs_minecraft_resource::ResourceKey;
use std::fmt;

use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_registries::{Named, Refs, Resolved, registry_value};
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::common::{MobEffectDetails, MobEffectInstance};
use crate::harness::{Sample, entry};
use crate::keys::Potion;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::{Decode, Encode, Result};
use std::io::Write;

/// The full map, or on read a bare potion id. Custom effects never carry a
/// hidden effect: vanilla hands out copies that leave it behind, so no encoder
/// ever sees one.
#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct PotionContents<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potion: Option<M::Id<Potion>>,
    #[serde(
        default,
        deserialize_with = "optional_int",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_color: Option<i32>,
    #[serde(
        default,
        deserialize_with = "effects_or_default",
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_without_hidden",
        bound(
            serialize = "MobEffectInstance<M>: Serialize",
            deserialize = "MobEffectInstance<M>: Deserialize<'de>"
        )
    )]
    pub custom_effects: Vec<MobEffectInstance<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
}

impl<M: Refs> Default for PotionContents<M> {
    fn default() -> Self {
        PotionContents {
            potion: None,
            custom_color: None,
            custom_effects: Vec::new(),
            custom_name: None,
        }
    }
}

fn effects_or_default<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

fn optional_int<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
    Ok(Option::<Bounded<{ i32::MIN }, { i32::MAX }>>::deserialize(d)?.map(|b| b.0))
}

impl<M: Refs> PotionContents<M> {
    pub fn potion(potion: M::Id<Potion>) -> Self {
        PotionContents {
            potion: Some(potion),
            ..PotionContents::default()
        }
    }

    pub fn visible_effects(&self) -> Vec<MobEffectInstance<M>> {
        let mut effects = self.custom_effects.clone();
        strip_hidden(&mut effects);
        effects
    }
}

pub fn strip_hidden<M: Refs>(effects: &mut [MobEffectInstance<M>]) {
    for effect in effects {
        effect.details.hidden_effect = None;
    }
}

fn serialize_without_hidden<M: Refs, S: Serializer>(
    effects: &[MobEffectInstance<M>],
    s: S,
) -> Result<S::Ok, S::Error>
where
    MobEffectInstance<M>: Serialize,
{
    let mut effects = effects.to_vec();
    strip_hidden(&mut effects);
    effects.serialize(s)
}

impl Serialize for PotionContents<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        PotionContents::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for PotionContents<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ContentsVisitor;

        impl<'de> Visitor<'de> for ContentsVisitor {
            type Value = PotionContents<Named>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("potion contents or a potion id")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                ResourceKey::deserialize(value::StrDeserializer::new(text))
                    .map(PotionContents::potion)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let mut contents =
                    PotionContents::deserialize(value::MapAccessDeserializer::new(map))?;
                strip_hidden(&mut contents.custom_effects);
                Ok(contents)
            }
        }

        d.deserialize_any(ContentsVisitor)
    }
}

impl Sample for PotionContents {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if self.potion.is_some() {
            tags.push(("potion", STRING_ID));
        }
        if self.custom_color.is_some() {
            tags.push(("custom_color", INT_ID));
        }
        if !self.custom_effects.is_empty() {
            tags.push(("custom_effects", LIST_ID));
        }
        if self.custom_name.is_some() {
            tags.push(("custom_name", STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        let effect = |path: &str, details: MobEffectDetails| MobEffectInstance {
            id: entry(path),
            details,
        };
        vec![
            PotionContents::default(),
            PotionContents::potion(entry("swiftness")),
            PotionContents {
                custom_color: Some(-13083194),
                ..PotionContents::default()
            },
            PotionContents {
                potion: Some(entry("water")),
                custom_color: Some(0xFF0000),
                custom_effects: vec![
                    effect(
                        "speed",
                        MobEffectDetails {
                            amplifier: 1,
                            duration: 100,
                            ..MobEffectDetails::default()
                        },
                    ),
                    effect(
                        "haste",
                        MobEffectDetails {
                            duration: -1,
                            ambient: true,
                            show_particles: false,
                            show_icon: true,
                            ..MobEffectDetails::default()
                        },
                    ),
                ],
                custom_name: Some("mcrs".into()),
            },
        ]
    }
}

impl Encode for PotionContents<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.potion.encode(&mut w)?;
        self.custom_color.encode(&mut w)?;
        self.visible_effects().encode(&mut w)?;
        self.custom_name.encode(w)
    }
}

impl Decode<'_> for PotionContents<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let potion = Option::decode(r)?;
        let custom_color = Option::decode(r)?;
        let mut custom_effects: Vec<MobEffectInstance<Wire>> = Vec::decode(r)?;
        strip_hidden(&mut custom_effects);
        Ok(PotionContents {
            potion,
            custom_color,
            custom_effects,
            custom_name: Option::decode(r)?,
        })
    }
}
