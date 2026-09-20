use std::fmt;
use std::io::Write;

use mcrs_minecraft_core::codec::int_value;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::common::{MobEffectDetails, MobEffectInstance, PotionReg};
use crate::item::component::registry_ref::null_as_default;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::{Decode, Encode};

/// `PotionContents.CODEC`: the full map, or on read a bare potion id. Custom
/// effects never carry a hidden effect: vanilla's `customEffects()` accessor
/// hands out copies made by `setDetailsFrom`, which leaves it behind, so no
/// encoder ever sees one.
#[derive(Clone, Debug, PartialEq, Default, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PotionContents {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub potion: Option<ResourceKey<PotionReg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<i32>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_without_hidden"
    )]
    pub custom_effects: Vec<MobEffectInstance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
}

impl PotionContents {
    pub fn potion(potion: ResourceKey<PotionReg>) -> Self {
        PotionContents {
            potion: Some(potion),
            ..PotionContents::default()
        }
    }

    fn visible_effects(&self) -> Vec<MobEffectInstance> {
        let mut effects = self.custom_effects.clone();
        strip_hidden(&mut effects);
        effects
    }
}

fn strip_hidden(effects: &mut [MobEffectInstance]) {
    for effect in effects {
        effect.details.hidden_effect = None;
    }
}

fn serialize_without_hidden<S: Serializer>(
    effects: &[MobEffectInstance],
    s: S,
) -> Result<S::Ok, S::Error> {
    let mut effects = effects.to_vec();
    strip_hidden(&mut effects);
    effects.serialize(s)
}

impl<'de> Deserialize<'de> for PotionContents {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Full {
            #[serde(default)]
            potion: Option<ResourceKey<PotionReg>>,
            #[serde(default, deserialize_with = "optional_int")]
            custom_color: Option<i32>,
            #[serde(default, deserialize_with = "effects_or_default")]
            custom_effects: Vec<MobEffectInstance>,
            #[serde(default)]
            custom_name: Option<String>,
        }

        null_as_default! {
            effects_or_default: Vec<MobEffectInstance> = Vec::new();
        }

        fn optional_int<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
            struct Int(i32);

            impl<'de> Deserialize<'de> for Int {
                fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    int_value(d).map(Int)
                }
            }

            Ok(Option::<Int>::deserialize(d)?.map(|Int(v)| v))
        }

        struct ContentsVisitor;

        impl<'de> Visitor<'de> for ContentsVisitor {
            type Value = PotionContents;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("potion contents or a potion id")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                ResourceLocation::read(text)
                    .map(|location| PotionContents::potion(ResourceKey::from_location(location)))
                    .map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let mut full = Full::deserialize(value::MapAccessDeserializer::new(map))?;
                strip_hidden(&mut full.custom_effects);
                Ok(PotionContents {
                    potion: full.potion,
                    custom_color: full.custom_color,
                    custom_effects: full.custom_effects,
                    custom_name: full.custom_name,
                })
            }
        }

        d.deserialize_any(ContentsVisitor)
    }
}

impl EncodeCtx for PotionContents {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.potion.encode_ctx(ctx, &mut w)?;
        self.custom_color.encode(&mut w)?;
        self.visible_effects().encode_ctx(ctx, &mut w)?;
        self.custom_name.encode(w)
    }
}

impl DecodeCtx<'_> for PotionContents {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let potion = Option::decode_ctx(ctx, r)?;
        let custom_color = Option::decode(r)?;
        let mut custom_effects: Vec<MobEffectInstance> = Vec::decode_ctx(ctx, r)?;
        strip_hidden(&mut custom_effects);
        Ok(PotionContents {
            potion,
            custom_color,
            custom_effects,
            custom_name: Option::decode(r)?,
        })
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
        fn key<R>(path: &str) -> ResourceKey<R> {
            ResourceKey::from_location(ResourceLocation::minecraft(path))
        }
        let effect = |path: &str, details: MobEffectDetails| MobEffectInstance {
            id: key(path),
            details,
        };
        vec![
            PotionContents::default(),
            PotionContents::potion(key("swiftness")),
            PotionContents {
                custom_color: Some(-13083194),
                ..PotionContents::default()
            },
            PotionContents {
                potion: Some(key("water")),
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
