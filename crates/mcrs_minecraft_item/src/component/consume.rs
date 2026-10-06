use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, rl};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_keys::MobEffect;
use mcrs_minecraft_nbt::nbt_flag;
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_sound::keys::sound_event;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::common::{
    Folded, Holder, ItemUseAnimation, MobEffectDetails, MobEffectInstance, list_set, one_set,
};
use crate::harness::Sample;
use mcrs_minecraft_sound::SoundEvent;

/// A ranged float field with vanilla's error wording; the bounds order `-0.0`
/// below `0.0` and NaN outside every range.
macro_rules! checked_float {
    ($($vis:vis $name:ident: $value:ident in $min:literal $op:tt $max:expr => $message:literal),* $(,)?) => {$(
        $vis fn $name<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
            let $value = ::mcrs_minecraft_core::codec::float_value(d)?;
            if $value.total_cmp(&$min).$op() && $value.total_cmp(&$max).is_le() {
                Ok($value)
            } else {
                let $value = ::mcrs_minecraft_nbt::snbt::java_float($value);
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
            Self::ApplyEffects => keys::consume_effect_type::APPLY_EFFECTS.as_static_str(),
            Self::RemoveEffects => keys::consume_effect_type::REMOVE_EFFECTS.as_static_str(),
            Self::ClearAllEffects => keys::consume_effect_type::CLEAR_ALL_EFFECTS.as_static_str(),
            Self::TeleportRandomly => keys::consume_effect_type::TELEPORT_RANDOMLY.as_static_str(),
            Self::PlaySound => keys::consume_effect_type::PLAY_SOUND.as_static_str(),
        }
    }

    pub fn from_wire_id(id: u16) -> Option<Self> {
        Self::ALL.get(usize::from(id)).copied()
    }
}

const _: () = assert!(mcrs_minecraft_registry::static_rows::rows_match(
    &{
        let mut rows = [("", 0); ConsumeEffectType::ALL.len()];
        let mut i = 0;
        while i < rows.len() {
            rows[i] = (
                ConsumeEffectType::ALL[i].id(),
                ConsumeEffectType::ALL[i] as u16,
            );
            i += 1;
        }
        rows
    },
    keys::consume_effect_type::ENTRIES,
    true
));

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
    RemoveEffects { effects: HolderSet<MobEffect> },
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

const CONSUME_EFFECT_TYPE_ROWS: &[&str] = &[
    "minecraft:apply_effects",
    "minecraft:remove_effects",
    "minecraft:clear_all_effects",
    "minecraft:teleport_randomly",
    "minecraft:play_sound",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    CONSUME_EFFECT_TYPE_ROWS,
    &[],
    keys::consume_effect_type::ENTRIES
));

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
            Self::RemoveEffects { effects } => map.serialize_entry("effects", &Folded(effects))?,
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
    #[serde(
        default = "default_true",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
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
    Holder::Reference(sound_event::ENTITY_GENERIC_EAT.id())
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

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathProtection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub death_effects: Vec<ConsumeEffect>,
}

pub fn every_consume_effect() -> Vec<ConsumeEffect> {
    let effect = |path: &str, details: MobEffectDetails| MobEffectInstance {
        id: ResourceKey::from_location(ResourceLocation::minecraft(path).unwrap()),
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
            sound: Holder::Reference(sound_event::ENTITY_ITEM_BREAK.id()),
        },
        ConsumeEffect::PlaySound {
            sound: Holder::Direct(SoundEvent {
                sound_id: rl!("mcrs:ding").to_arc(),
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
                    sound_id: rl!("mcrs:sip").to_arc(),
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

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn consume_effect_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<ConsumeEffect>(
            CONSUME_EFFECT_TYPE_ROWS,
            &[],
            keys::consume_effect_type::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
