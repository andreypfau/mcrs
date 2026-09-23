use std::collections::BTreeMap;
use std::ops::Not;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{Bounded, NonNegativeInt, default_true};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, FLOAT_ID, INT_ID, LIST_ID, STRING_ID};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::common::{self, RgbInt};
use crate::component::consume::{
    checked_float, float_default, is_one, is_zero, non_negative_float, one, positive_float,
    unit_float, zero,
};
use crate::harness::Sample;
use crate::kind::ItemComponentKind;

float_default! {
    speed_multiplier / is_speed_multiplier = 0.2f32,
    three / is_three = 3.0f32,
    five / is_five = 5.0f32,
    hitbox_margin / is_hitbox_margin = 0.3f32,
}

checked_float! {
    mob_factor: v in 0.0 is_ge 2.0 => "Value {v} outside of range [0.0:2.0]",
    reach: v in 0.0 is_ge 64.0 => "Value must be within range [0.0;64.0]: {v}",
    margin: v in 0.0 is_ge 1.0 => "Value must be within range [0.0;1.0]: {v}",
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseEffects {
    #[serde(default, skip_serializing_if = "Not::not")]
    pub can_sprint: bool,
    #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
    pub interact_vibrations: bool,
    #[serde(
        default = "speed_multiplier",
        deserialize_with = "unit_float",
        skip_serializing_if = "is_speed_multiplier"
    )]
    pub speed_multiplier: f32,
}

impl Default for UseEffects {
    fn default() -> Self {
        UseEffects {
            can_sprint: false,
            interact_vibrations: true,
            speed_multiplier: 0.2,
        }
    }
}

impl Sample for UseEffects {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if self.can_sprint {
            tags.extend([
                ("can_sprint", BYTE_ID),
                ("interact_vibrations", BYTE_ID),
                ("speed_multiplier", FLOAT_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            UseEffects::default(),
            UseEffects {
                can_sprint: true,
                interact_vibrations: false,
                speed_multiplier: 0.5,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomModelData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub floats: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<RgbInt>,
}

impl Sample for CustomModelData {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !self.floats.is_empty() {
            tags.extend([
                ("floats", LIST_ID),
                ("flags", LIST_ID),
                ("strings", LIST_ID),
                ("colors", LIST_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            CustomModelData::default(),
            CustomModelData {
                floats: vec![1.5, -2.0],
                flags: vec![true, false],
                strings: vec!["a".into(), "b".into()],
                colors: vec![RgbInt(0xFF0000), RgbInt(-16711936)],
            },
        ]
    }
}

/// A linked set: the order is kept, a repeat is dropped.
fn distinct(kinds: Vec<ItemComponentKind>) -> Vec<ItemComponentKind> {
    let mut seen = Vec::with_capacity(kinds.len());
    for kind in kinds {
        if !seen.contains(&kind) {
            seen.push(kind);
        }
    }
    seen
}

/// An unknown kind is worded as a registry lookup failure, unlike a patch's
/// unknown key.
fn distinct_kinds<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ItemComponentKind>, D::Error> {
    Vec::<ResourceLocation>::deserialize(d)?
        .iter()
        .map(|id| {
            ItemComponentKind::from_id(id.as_str()).ok_or_else(|| {
                D::Error::custom(format_args!(
                    "Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"
                ))
            })
        })
        .collect::<Result<_, _>>()
        .map(distinct)
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TooltipDisplay {
    #[serde(default, skip_serializing_if = "Not::not")]
    pub hide_tooltip: bool,
    #[serde(
        default,
        deserialize_with = "distinct_kinds",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub hidden_components: Vec<ItemComponentKind>,
}

impl TooltipDisplay {
    pub fn new(hide_tooltip: bool, hidden_components: Vec<ItemComponentKind>) -> Self {
        TooltipDisplay {
            hide_tooltip,
            hidden_components: distinct(hidden_components),
        }
    }
}

impl Sample for TooltipDisplay {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if self.hide_tooltip {
            tags.extend([("hide_tooltip", BYTE_ID), ("hidden_components", LIST_ID)]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            TooltipDisplay::default(),
            TooltipDisplay::new(
                true,
                vec![
                    ItemComponentKind::Enchantments,
                    ItemComponentKind::Lore,
                    ItemComponentKind::CreativeSlotLock,
                ],
            ),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Food {
    pub nutrition: NonNegativeInt,
    pub saturation: f32,
    #[serde(default, skip_serializing_if = "Not::not")]
    pub can_always_eat: bool,
}

impl Sample for Food {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![
            ("", COMPOUND_ID),
            ("nutrition", INT_ID),
            ("saturation", FLOAT_ID),
        ];
        if self.can_always_eat {
            tags.push(("can_always_eat", BYTE_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Food {
                nutrition: Bounded(4),
                saturation: 2.4,
                can_always_eat: false,
            },
            Food {
                nutrition: Bounded(1),
                saturation: 0.6,
                can_always_eat: true,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseCooldown {
    #[serde(deserialize_with = "positive_float")]
    pub seconds: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_group: Option<ResourceLocation>,
}

impl Sample for UseCooldown {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID), ("seconds", FLOAT_ID)];
        if self.cooldown_group.is_some() {
            tags.push(("cooldown_group", STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            UseCooldown {
                seconds: 1.5,
                cooldown_group: None,
            },
            UseCooldown {
                seconds: 0.5,
                cooldown_group: Some(ResourceLocation::minecraft("ender_pearl")),
            },
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weapon {
    #[serde(default = "common::one", skip_serializing_if = "common::is_one")]
    pub item_damage_per_attack: NonNegativeInt,
    #[serde(
        default = "zero",
        deserialize_with = "non_negative_float",
        skip_serializing_if = "is_zero"
    )]
    pub disable_blocking_for_seconds: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Weapon {
            item_damage_per_attack: common::one(),
            disable_blocking_for_seconds: 0.0,
        }
    }
}

impl Sample for Weapon {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if *self != Weapon::default() {
            tags.extend([
                ("item_damage_per_attack", INT_ID),
                ("disable_blocking_for_seconds", FLOAT_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Weapon::default(),
            Weapon {
                item_damage_per_attack: Bounded(2),
                disable_blocking_for_seconds: 5.0,
            },
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackRange {
    #[serde(
        default = "zero",
        deserialize_with = "reach",
        skip_serializing_if = "is_zero"
    )]
    pub min_reach: f32,
    #[serde(
        default = "three",
        deserialize_with = "reach",
        skip_serializing_if = "is_three"
    )]
    pub max_reach: f32,
    #[serde(
        default = "zero",
        deserialize_with = "reach",
        skip_serializing_if = "is_zero"
    )]
    pub min_creative_reach: f32,
    #[serde(
        default = "five",
        deserialize_with = "reach",
        skip_serializing_if = "is_five"
    )]
    pub max_creative_reach: f32,
    #[serde(
        default = "hitbox_margin",
        deserialize_with = "margin",
        skip_serializing_if = "is_hitbox_margin"
    )]
    pub hitbox_margin: f32,
    #[serde(
        default = "one",
        deserialize_with = "mob_factor",
        skip_serializing_if = "is_one"
    )]
    pub mob_factor: f32,
}

impl Default for AttackRange {
    fn default() -> Self {
        AttackRange {
            min_reach: 0.0,
            max_reach: 3.0,
            min_creative_reach: 0.0,
            max_creative_reach: 5.0,
            hitbox_margin: 0.3,
            mob_factor: 1.0,
        }
    }
}

impl Sample for AttackRange {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if *self != AttackRange::default() {
            tags.extend([
                ("min_reach", FLOAT_ID),
                ("max_reach", FLOAT_ID),
                ("min_creative_reach", FLOAT_ID),
                ("max_creative_reach", FLOAT_ID),
                ("hitbox_margin", FLOAT_ID),
                ("mob_factor", FLOAT_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            AttackRange::default(),
            AttackRange {
                min_reach: 1.0,
                max_reach: 4.0,
                min_creative_reach: 0.5,
                max_creative_reach: 6.0,
                hitbox_margin: 0.1,
                mob_factor: 1.5,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockState(pub BTreeMap<String, String>);

impl Sample for BlockState {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !self.0.is_empty() {
            tags.push(("facing", STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            BlockState::default(),
            BlockState(BTreeMap::from([
                ("facing".to_string(), "north".to_string()),
                ("lit".to_string(), "true".to_string()),
            ])),
        ]
    }
}
