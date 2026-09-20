use std::cmp::Ordering;
use std::io::Write;

use anyhow::bail;
use mcrs_minecraft_core::codec::{self, NonNegativeInt, default_true, float_value, int_value};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::nbt_flag;
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::Error as _;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::common::{
    Holder, ItemUseAnimation, MobEffectDetails, MobEffectInstance, MobEffectReg,
};
use crate::item::component::sound::SoundEvent;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::{Decode, Encode, VarInt};

/// Java's `Float.toString` for the values an error message can carry.
fn java_float(value: f32) -> String {
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if value.fract() == 0.0 && value.abs() < 1e7 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

/// `-0.0` and NaN are out of range.
pub(crate) fn positive_float<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let value = float_value(d)?;
    if value.total_cmp(&0.0) != Ordering::Greater || value.total_cmp(&f32::MAX) == Ordering::Greater
    {
        return Err(D::Error::custom(format_args!(
            "Value must be positive: {}",
            java_float(value)
        )));
    }
    Ok(value)
}

pub(crate) fn non_negative_float<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let value = float_value(d)?;
    if value.total_cmp(&0.0) == Ordering::Less || value.total_cmp(&f32::MAX) == Ordering::Greater {
        return Err(D::Error::custom(format_args!(
            "Value must be non-negative: {}",
            java_float(value)
        )));
    }
    Ok(value)
}

/// The core alias's bound, with vanilla's error wording.
pub(crate) fn non_negative_int<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<NonNegativeInt, D::Error> {
    let value = int_value(d)?;
    if value < 0 {
        return Err(D::Error::custom(format_args!(
            "Value must be non-negative: {value}"
        )));
    }
    Ok(codec::Bounded(value))
}

fn unit_float<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let value = float_value(d)?;
    if value.total_cmp(&0.0) == Ordering::Less || value.total_cmp(&1.0) == Ordering::Greater {
        return Err(D::Error::custom(format_args!(
            "Value {} outside of range [0.0:1.0]",
            java_float(value)
        )));
    }
    Ok(value)
}

/// The default is compared by bits, so `-0.0` is still written.
macro_rules! float_default {
    ($($default:ident / $is:ident = $value:literal),* $(,)?) => {$(
        pub(crate) fn $default() -> f32 {
            $value
        }

        pub(crate) fn $is(value: &f32) -> bool {
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

pub(crate) fn is_true(value: &bool) -> bool {
    *value
}

/// `minecraft:consume_effect_type`, whose ids are the wire dispatch prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConsumeEffectType {
    ApplyEffects = 0,
    RemoveEffects = 1,
    ClearAllEffects = 2,
    TeleportRandomly = 3,
    PlaySound = 4,
}

impl ConsumeEffectType {
    pub const ALL: [Self; 5] = [
        Self::ApplyEffects,
        Self::RemoveEffects,
        Self::ClearAllEffects,
        Self::TeleportRandomly,
        Self::PlaySound,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::ApplyEffects => "minecraft:apply_effects",
            Self::RemoveEffects => "minecraft:remove_effects",
            Self::ClearAllEffects => "minecraft:clear_all_effects",
            Self::TeleportRandomly => "minecraft:teleport_randomly",
            Self::PlaySound => "minecraft:play_sound",
        }
    }

    fn from_wire_id(id: i32) -> Option<Self> {
        usize::try_from(id)
            .ok()
            .and_then(|id| Self::ALL.get(id))
            .copied()
    }
}

/// The map buffers on read, so the bools inside go through `nbt_flag`;
/// vanilla writes the dispatch key last, so `Serialize` is by hand.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ConsumeEffect {
    #[serde(rename = "minecraft:apply_effects", alias = "apply_effects")]
    ApplyEffects {
        effects: Vec<MobEffectInstance>,
        #[serde(default = "one", deserialize_with = "unit_float")]
        probability: f32,
    },
    #[serde(rename = "minecraft:remove_effects", alias = "remove_effects")]
    RemoveEffects {
        effects: HolderSet<ResourceKey<MobEffectReg>>,
    },
    #[serde(rename = "minecraft:clear_all_effects", alias = "clear_all_effects")]
    ClearAllEffects,
    #[serde(rename = "minecraft:teleport_randomly", alias = "teleport_randomly")]
    TeleportRandomly {
        #[serde(default = "sixteen", deserialize_with = "positive_float")]
        diameter: f32,
        #[serde(default = "default_true", deserialize_with = "nbt_flag")]
        directional_particles: bool,
    },
    #[serde(rename = "minecraft:play_sound", alias = "play_sound")]
    PlaySound { sound: Holder<SoundEvent> },
}

impl ConsumeEffect {
    pub fn kind(&self) -> ConsumeEffectType {
        match self {
            Self::ApplyEffects { .. } => ConsumeEffectType::ApplyEffects,
            Self::RemoveEffects { .. } => ConsumeEffectType::RemoveEffects,
            Self::ClearAllEffects => ConsumeEffectType::ClearAllEffects,
            Self::TeleportRandomly { .. } => ConsumeEffectType::TeleportRandomly,
            Self::PlaySound { .. } => ConsumeEffectType::PlaySound,
        }
    }
}

impl Serialize for ConsumeEffect {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        match self {
            Self::ApplyEffects {
                effects,
                probability,
            } => {
                map.serialize_entry("effects", effects)?;
                if !is_one(probability) {
                    map.serialize_entry("probability", probability)?;
                }
            }
            Self::RemoveEffects { effects } => map.serialize_entry("effects", effects)?,
            Self::ClearAllEffects => {}
            Self::TeleportRandomly {
                diameter,
                directional_particles,
            } => {
                if !is_sixteen(diameter) {
                    map.serialize_entry("diameter", diameter)?;
                }
                if !directional_particles {
                    map.serialize_entry("directional_particles", directional_particles)?;
                }
            }
            Self::PlaySound { sound } => map.serialize_entry("sound", sound)?,
        }
        map.serialize_entry("type", self.kind().id())?;
        map.end()
    }
}

impl EncodeCtx for ConsumeEffect {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.kind() as i32).encode(&mut w)?;
        match self {
            Self::ApplyEffects {
                effects,
                probability,
            } => {
                effects.encode_ctx(ctx, &mut w)?;
                probability.encode(w)
            }
            Self::RemoveEffects { effects } => effects.encode_ctx(ctx, w),
            Self::ClearAllEffects => Ok(()),
            Self::TeleportRandomly {
                diameter,
                directional_particles,
            } => {
                diameter.encode(&mut w)?;
                directional_particles.encode(w)
            }
            Self::PlaySound { sound } => sound.encode_ctx(ctx, w),
        }
    }
}

impl DecodeCtx<'_> for ConsumeEffect {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let Some(kind) = ConsumeEffectType::from_wire_id(id) else {
            bail!("unknown consume effect type {id}");
        };
        Ok(match kind {
            ConsumeEffectType::ApplyEffects => Self::ApplyEffects {
                effects: Vec::decode_ctx(ctx, r)?,
                probability: f32::decode(r)?,
            },
            ConsumeEffectType::RemoveEffects => Self::RemoveEffects {
                effects: HolderSet::decode_ctx(ctx, r)?,
            },
            ConsumeEffectType::ClearAllEffects => Self::ClearAllEffects,
            ConsumeEffectType::TeleportRandomly => Self::TeleportRandomly {
                diameter: f32::decode(r)?,
                directional_particles: bool::decode(r)?,
            },
            ConsumeEffectType::PlaySound => Self::PlaySound {
                sound: Holder::decode_ctx(ctx, r)?,
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consumable {
    #[serde(
        default = "consume_seconds_default",
        deserialize_with = "non_negative_float",
        skip_serializing_if = "is_consume_seconds_default"
    )]
    pub consume_seconds: f32,
    #[serde(default = "eat", skip_serializing_if = "is_eat")]
    pub animation: ItemUseAnimation,
    #[serde(default = "generic_eat", skip_serializing_if = "is_generic_eat")]
    pub sound: Holder<SoundEvent>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub has_consume_particles: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_consume_effects: Vec<ConsumeEffect>,
}

fn eat() -> ItemUseAnimation {
    ItemUseAnimation::Eat
}

fn is_eat(animation: &ItemUseAnimation) -> bool {
    *animation == ItemUseAnimation::Eat
}

fn generic_eat() -> Holder<SoundEvent> {
    Holder::reference(ResourceLocation::minecraft("entity.generic.eat"))
}

fn is_generic_eat(sound: &Holder<SoundEvent>) -> bool {
    *sound == generic_eat()
}

impl Default for Consumable {
    fn default() -> Self {
        Consumable {
            consume_seconds: consume_seconds_default(),
            animation: eat(),
            sound: generic_eat(),
            has_consume_particles: true,
            on_consume_effects: Vec::new(),
        }
    }
}

impl EncodeCtx for Consumable {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.consume_seconds.encode(&mut w)?;
        self.animation.encode(&mut w)?;
        self.sound.encode_ctx(ctx, &mut w)?;
        self.has_consume_particles.encode(&mut w)?;
        self.on_consume_effects.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Consumable {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Consumable {
            consume_seconds: f32::decode(r)?,
            animation: ItemUseAnimation::decode(r)?,
            sound: Holder::decode_ctx(ctx, r)?,
            has_consume_particles: bool::decode(r)?,
            on_consume_effects: Vec::decode_ctx(ctx, r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathProtection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub death_effects: Vec<ConsumeEffect>,
}

impl EncodeCtx for DeathProtection {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.death_effects.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for DeathProtection {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(DeathProtection {
            death_effects: Vec::decode_ctx(ctx, r)?,
        })
    }
}

pub(crate) fn every_consume_effect() -> Vec<ConsumeEffect> {
    let effect = |path: &str, details: MobEffectDetails| MobEffectInstance {
        id: ResourceKey::from_location(ResourceLocation::minecraft(path)),
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
            effects: HolderSet::List(vec![
                ResourceKey::from_location(ResourceLocation::minecraft("speed")),
                ResourceKey::from_location(ResourceLocation::minecraft("slowness")),
            ]),
        },
        ConsumeEffect::RemoveEffects {
            effects: HolderSet::One(ResourceKey::from_location(ResourceLocation::minecraft(
                "haste",
            ))),
        },
        ConsumeEffect::ClearAllEffects,
        ConsumeEffect::TeleportRandomly {
            diameter: 8.0,
            directional_particles: false,
        },
        ConsumeEffect::PlaySound {
            sound: Holder::reference(ResourceLocation::minecraft("entity.item.break")),
        },
        ConsumeEffect::PlaySound {
            sound: Holder::Direct(SoundEvent {
                sound_id: ResourceLocation::new("mcrs", "ding"),
                range: None,
            }),
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
                sound: Holder::Direct(SoundEvent {
                    sound_id: ResourceLocation::new("mcrs", "sip"),
                    range: Some(8.0),
                }),
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
