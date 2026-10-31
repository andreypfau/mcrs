use std::sync::Arc;

use crate::keys::MobEffect;
use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_nbt::nbt_flag;
use mcrs_minecraft_registries::{Ref, Refs, Resolved, dispatch, registry_value};
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_sound::keys::SoundEvent;
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::common::{ItemUseAnimation, MobEffectDetails, MobEffectInstance};
use crate::harness::Sample;
use crate::harness::{entry, list_set, one_set};
use crate::keys::ConsumeEffectType;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_registries::{decode_registry_id, encode_registry_id};
use mcrs_minecraft_sound::{Sound, SoundEventValue};
use mcrs_minecraft_wire::{Decode, Encode, Result, bail};
use std::io::Write;

/// A ranged float field with vanilla's error wording; the bounds order `-0.0`
/// below `0.0` and NaN outside every range.
macro_rules! checked_float {
    ($($vis:vis $name:ident: $value:ident in $min:literal $op:tt $max:expr => $message:literal),* $(,)?) => {$(
        $vis fn $name<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
            let $value = ::mcrs_minecraft_core::codec::float_value(d)?;
            if $value.total_cmp(&$min).$op() && $value.total_cmp(&$max).is_le() {
                Ok($value)
            } else {
                let $value = ::mcrs_minecraft_snbt::java_float($value);
                Err(<D::Error as ::serde::de::Error>::custom(format_args!($message)))
            }
        }
    )*};
}
pub(crate) use checked_float;

checked_float! {
    pub positive_float: v in 0.0 is_gt f32::MAX => "Value must be positive: {v}",
    pub non_negative_float: v in 0.0 is_ge f32::MAX => "Value must be non-negative: {v}",
    pub unit_float: v in 0.0 is_ge 1.0 => "Value {v} outside of range [0.0:1.0]",
}

/// The default is compared by bits, so `-0.0` is still written.
macro_rules! float_default {
    ($($default:ident / $is:ident = $value:literal),* $(,)?) => {$(
        pub fn $default() -> f32 {
            $value
        }

        pub fn $is(value: &f32) -> bool {
            value.to_bits() == $value.to_bits()
        }
    )*};
}
pub(crate) use float_default;

float_default! {
    zero / is_zero = 0.0f32,
    one / is_one = 1.0f32,
    sixteen / is_sixteen = 16.0f32,
    consume_seconds_default / is_consume_seconds_default = 1.6f32,
}

/// The map buffers on read, so the bools inside go through `nbt_flag`.
#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire)]
#[dispatch(registry = crate::keys::ConsumeEffectType, key_last)]
#[derive(Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum ConsumeEffect<M: Refs = Resolved> {
    ApplyEffects {
        effects: Vec<MobEffectInstance<M>>,
        #[serde(
            default = "one",
            deserialize_with = "unit_float",
            skip_serializing_if = "is_one"
        )]
        probability: f32,
    },
    RemoveEffects {
        effects: M::Set<MobEffect>,
    },
    ClearAllEffects,
    TeleportRandomly {
        #[serde(
            default = "sixteen",
            deserialize_with = "positive_float",
            skip_serializing_if = "is_sixteen"
        )]
        diameter: f32,
        #[serde(
            default = "default_true",
            deserialize_with = "nbt_flag",
            skip_serializing_if = "std::clone::Clone::clone"
        )]
        directional_particles: bool,
    },
    PlaySound {
        sound: Sound<M>,
    },
}

#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(deny_unknown_fields)]
pub struct Consumable<M: Refs = Resolved> {
    #[serde(
        default = "consume_seconds_default",
        deserialize_with = "non_negative_float",
        skip_serializing_if = "is_consume_seconds_default"
    )]
    pub consume_seconds: f32,
    #[serde(default = "eat", skip_serializing_if = "is_eat")]
    pub animation: ItemUseAnimation,
    #[serde(
        default = "generic_eat::<M>",
        skip_serializing_if = "is_generic_eat::<M>"
    )]
    pub sound: Sound<M>,
    #[serde(
        default = "default_true",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub has_consume_particles: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_consume_effects: Vec<ConsumeEffect<M>>,
}

fn eat() -> ItemUseAnimation {
    ItemUseAnimation::Eat
}

fn is_eat(animation: &ItemUseAnimation) -> bool {
    *animation == ItemUseAnimation::Eat
}

fn generic_eat<M: Refs>() -> Sound<M> {
    Ref::of(SoundEvent::EntityGenericEat)
}

fn is_generic_eat<M: Refs>(sound: &Sound<M>) -> bool {
    *sound == generic_eat::<M>()
}

impl<M: Refs> Default for Consumable<M> {
    fn default() -> Self {
        Consumable {
            consume_seconds: consume_seconds_default(),
            animation: eat(),
            sound: generic_eat::<M>(),
            has_consume_particles: true,
            on_consume_effects: Vec::new(),
        }
    }
}

#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire,
    Clone,
    Debug,
    PartialEq,
    Default,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(deny_unknown_fields)]
pub struct DeathProtection<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub death_effects: Vec<ConsumeEffect<M>>,
}

pub fn every_consume_effect() -> Vec<ConsumeEffect> {
    let effect = |path: &str, details: MobEffectDetails| MobEffectInstance {
        id: entry(path),
        details,
    };
    vec![
        ConsumeEffect::ApplyEffects {
            effects: vec![
                effect(
                    "speed",
                    MobEffectDetails {
                        amplifier: 1,
                        duration: 100,
                        ..Default::default()
                    },
                ),
                effect(
                    "slowness",
                    MobEffectDetails {
                        duration: 20,
                        ambient: true,
                        show_particles: false,
                        show_icon: false,
                        hidden_effect: Some(Box::new(MobEffectDetails {
                            duration: 5,
                            ..Default::default()
                        })),
                        ..Default::default()
                    },
                ),
            ],
            probability: 0.5,
        },
        ConsumeEffect::RemoveEffects {
            effects: list_set(&["speed", "slowness"]),
        },
        ConsumeEffect::RemoveEffects {
            effects: one_set("haste"),
        },
        ConsumeEffect::ClearAllEffects,
        ConsumeEffect::TeleportRandomly {
            diameter: 8.0,
            directional_particles: false,
        },
        ConsumeEffect::PlaySound {
            sound: Ref::Id(SoundEvent::EntityItemBreak.id()),
        },
        ConsumeEffect::PlaySound {
            sound: Ref::Inline(Arc::new(SoundEventValue {
                sound_id: rl!("mcrs:ding").to_buf(),
                range: None,
            })),
        },
    ]
}

impl Sample for Consumable {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, FLOAT_ID, LIST_ID, STRING_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        if !is_consume_seconds_default(&self.consume_seconds) {
            tags.push(("consume_seconds", FLOAT_ID));
        }
        if !is_eat(&self.animation) {
            tags.push(("animation", STRING_ID));
        }
        if !self.has_consume_particles {
            tags.push(("has_consume_particles", BYTE_ID));
        }
        if !self.on_consume_effects.is_empty() {
            tags.push(("on_consume_effects", LIST_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Consumable::default(),
            Consumable {
                consume_seconds: 2.5,
                animation: ItemUseAnimation::Drink,
                sound: Ref::Inline(Arc::new(SoundEventValue {
                    sound_id: rl!("mcrs:sip").to_buf(),
                    range: Some(8.0),
                })),
                has_consume_particles: false,
                on_consume_effects: every_consume_effect(),
            },
            Consumable {
                on_consume_effects: vec![ConsumeEffect::TeleportRandomly {
                    diameter: 16.0,
                    directional_particles: true,
                }],
                ..Default::default()
            },
        ]
    }
}

impl Sample for DeathProtection {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", mcrs_minecraft_nbt::COMPOUND_ID)];
        if !self.death_effects.is_empty() {
            tags.push(("death_effects", mcrs_minecraft_nbt::LIST_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            DeathProtection::default(),
            DeathProtection {
                death_effects: every_consume_effect(),
            },
        ]
    }
}

impl Encode for ConsumeEffect<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        encode_registry_id(self.kind() as u16, &mut w)?;
        match self {
            Self::ApplyEffects {
                effects,
                probability,
            } => {
                effects.encode(&mut w)?;
                probability.encode(w)
            }
            Self::RemoveEffects { effects } => effects.encode(w),
            Self::ClearAllEffects => Ok(()),
            Self::TeleportRandomly {
                diameter,
                directional_particles,
            } => {
                diameter.encode(&mut w)?;
                directional_particles.encode(w)
            }
            Self::PlaySound { sound } => sound.encode(w),
        }
    }
}

impl Decode<'_> for ConsumeEffect<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let id = decode_registry_id(r)?;
        let Some(kind) = ConsumeEffectType::from_protocol_id(id) else {
            bail!("unknown consume effect type {id}");
        };
        Ok(match kind {
            ConsumeEffectType::ApplyEffects => Self::ApplyEffects {
                effects: Vec::decode(r)?,
                probability: f32::decode(r)?,
            },
            ConsumeEffectType::RemoveEffects => Self::RemoveEffects {
                effects: Decode::decode(r)?,
            },
            ConsumeEffectType::ClearAllEffects => Self::ClearAllEffects,
            ConsumeEffectType::TeleportRandomly => Self::TeleportRandomly {
                diameter: f32::decode(r)?,
                directional_particles: bool::decode(r)?,
            },
            ConsumeEffectType::PlaySound => Self::PlaySound {
                sound: Decode::decode(r)?,
            },
        })
    }
}
