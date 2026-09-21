use mcrs_minecraft_core::codec::{self, NonNegativeInt, default_true, is_default};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use serde::{Deserialize, Serialize};

use crate::item::component::common::{DamageTypeReg, Holder};
use crate::item::component::consume::{
    float_default, is_one, is_true, is_zero, non_negative_float, one, positive_float, zero,
};
use crate::item::component::sound::SoundEvent;
use crate::item::harness::Sample;

float_default! {
    ninety / is_ninety = 90.0f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlocksAttacks {
    #[serde(
        default = "zero",
        deserialize_with = "non_negative_float",
        skip_serializing_if = "is_zero"
    )]
    pub block_delay_seconds: f32,
    #[serde(
        default = "one",
        deserialize_with = "non_negative_float",
        skip_serializing_if = "is_one"
    )]
    pub disable_cooldown_scale: f32,
    #[serde(
        default = "default_reductions",
        skip_serializing_if = "is_default_reductions"
    )]
    pub damage_reductions: Vec<DamageReduction>,
    #[serde(default, skip_serializing_if = "ItemDamageFunction::is_default")]
    pub item_damage: ItemDamageFunction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypassed_by: Option<HolderSet<ResourceKey<DamageTypeReg>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_sound: Option<Holder<SoundEvent>>,
    #[serde(
        default,
        rename = "disabled_sound",
        skip_serializing_if = "Option::is_none"
    )]
    pub disable_sound: Option<Holder<SoundEvent>>,
}

impl Default for BlocksAttacks {
    fn default() -> Self {
        BlocksAttacks {
            block_delay_seconds: 0.0,
            disable_cooldown_scale: 1.0,
            damage_reductions: default_reductions(),
            item_damage: ItemDamageFunction::default(),
            bypassed_by: None,
            block_sound: None,
            disable_sound: None,
        }
    }
}

fn default_reductions() -> Vec<DamageReduction> {
    vec![DamageReduction::default()]
}

fn is_default_reductions(reductions: &[DamageReduction]) -> bool {
    matches!(reductions, [only] if only.same_bits(&DamageReduction::default()))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageReduction {
    #[serde(
        default = "ninety",
        deserialize_with = "positive_float",
        skip_serializing_if = "is_ninety"
    )]
    pub horizontal_blocking_angle: f32,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub types: Option<HolderSet<ResourceKey<DamageTypeReg>>>,
    pub base: f32,
    pub factor: f32,
}

impl Default for DamageReduction {
    fn default() -> Self {
        DamageReduction {
            horizontal_blocking_angle: 90.0,
            types: None,
            base: 0.0,
            factor: 1.0,
        }
    }
}

impl DamageReduction {
    fn same_bits(&self, other: &Self) -> bool {
        self.horizontal_blocking_angle.to_bits() == other.horizontal_blocking_angle.to_bits()
            && self.types == other.types
            && self.base.to_bits() == other.base.to_bits()
            && self.factor.to_bits() == other.factor.to_bits()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDamageFunction {
    #[serde(deserialize_with = "non_negative_float")]
    pub threshold: f32,
    pub base: f32,
    pub factor: f32,
}

impl Default for ItemDamageFunction {
    fn default() -> Self {
        ItemDamageFunction {
            threshold: 1.0,
            base: 0.0,
            factor: 1.0,
        }
    }
}

impl ItemDamageFunction {
    fn is_default(&self) -> bool {
        let default = Self::default();
        self.threshold.to_bits() == default.threshold.to_bits()
            && self.base.to_bits() == default.base.to_bits()
            && self.factor.to_bits() == default.factor.to_bits()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PiercingWeapon {
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub deals_knockback: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dismounts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<Holder<SoundEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_sound: Option<Holder<SoundEvent>>,
}

impl Default for PiercingWeapon {
    fn default() -> Self {
        PiercingWeapon {
            deals_knockback: true,
            dismounts: false,
            sound: None,
            hit_sound: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KineticWeapon {
    #[serde(default = "ten", skip_serializing_if = "is_ten")]
    pub contact_cooldown_ticks: NonNegativeInt,
    #[serde(default, skip_serializing_if = "is_default")]
    pub delay_ticks: NonNegativeInt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismount_conditions: Option<KineticCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knockback_conditions: Option<KineticCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_conditions: Option<KineticCondition>,
    #[serde(default = "zero", skip_serializing_if = "is_zero")]
    pub forward_movement: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub damage_multiplier: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<Holder<SoundEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_sound: Option<Holder<SoundEvent>>,
}

fn ten() -> NonNegativeInt {
    codec::Bounded(10)
}

fn is_ten(ticks: &NonNegativeInt) -> bool {
    ticks.0 == 10
}

impl Default for KineticWeapon {
    fn default() -> Self {
        KineticWeapon {
            contact_cooldown_ticks: ten(),
            delay_ticks: codec::Bounded(0),
            dismount_conditions: None,
            knockback_conditions: None,
            damage_conditions: None,
            forward_movement: 0.0,
            damage_multiplier: 1.0,
            sound: None,
            hit_sound: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KineticCondition {
    pub max_duration_ticks: NonNegativeInt,
    #[serde(default = "zero", skip_serializing_if = "is_zero")]
    pub min_speed: f32,
    #[serde(default = "zero", skip_serializing_if = "is_zero")]
    pub min_relative_speed: f32,
}

fn item_break() -> Holder<SoundEvent> {
    Holder::reference(ResourceLocation::minecraft("entity.item.break"))
}

fn damage_type(path: &str) -> ResourceKey<DamageTypeReg> {
    ResourceKey::from_location(ResourceLocation::minecraft(path))
}

impl Sample for BlocksAttacks {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, LIST_ID, STRING_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        if !is_zero(&self.block_delay_seconds) {
            tags.push(("block_delay_seconds", FLOAT_ID));
        }
        if !is_default_reductions(&self.damage_reductions) {
            tags.push(("damage_reductions", LIST_ID));
        }
        if !self.item_damage.is_default() {
            tags.extend([
                ("item_damage", COMPOUND_ID),
                ("item_damage.threshold", FLOAT_ID),
            ]);
        }
        if self.bypassed_by.is_some() {
            tags.push(("bypassed_by", STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            BlocksAttacks::default(),
            BlocksAttacks {
                block_delay_seconds: 0.25,
                disable_cooldown_scale: 2.0,
                damage_reductions: vec![
                    DamageReduction {
                        horizontal_blocking_angle: 45.0,
                        types: Some(HolderSet::Tag(ResourceLocation::minecraft(
                            "bypasses_shield",
                        ))),
                        base: 1.0,
                        factor: 0.5,
                    },
                    DamageReduction {
                        types: Some(HolderSet::List(vec![
                            damage_type("in_fire"),
                            damage_type("lava"),
                        ])),
                        ..Default::default()
                    },
                    DamageReduction {
                        types: Some(HolderSet::One(damage_type("lava"))),
                        base: 2.0,
                        factor: 0.0,
                        ..Default::default()
                    },
                ],
                item_damage: ItemDamageFunction {
                    threshold: 3.0,
                    base: 1.0,
                    factor: 0.25,
                },
                bypassed_by: Some(HolderSet::One(damage_type("in_fire"))),
                block_sound: Some(item_break()),
                disable_sound: Some(Holder::Direct(SoundEvent {
                    sound_id: ResourceLocation::new("mcrs", "off"),
                    range: None,
                })),
            },
        ]
    }
}

impl Sample for PiercingWeapon {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", mcrs_minecraft_nbt::COMPOUND_ID)];
        if self.dismounts {
            tags.push(("dismounts", mcrs_minecraft_nbt::BYTE_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            PiercingWeapon::default(),
            PiercingWeapon {
                deals_knockback: false,
                dismounts: true,
                sound: Some(item_break()),
                hit_sound: Some(Holder::Direct(SoundEvent {
                    sound_id: ResourceLocation::new("mcrs", "hit"),
                    range: Some(4.0),
                })),
            },
        ]
    }
}

impl Sample for KineticWeapon {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, INT_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        if !is_ten(&self.contact_cooldown_ticks) {
            tags.push(("contact_cooldown_ticks", INT_ID));
        }
        if self.damage_conditions.is_some() {
            tags.extend([
                ("damage_conditions", COMPOUND_ID),
                ("damage_conditions.max_duration_ticks", INT_ID),
                ("damage_conditions.min_speed", FLOAT_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            KineticWeapon::default(),
            KineticWeapon {
                contact_cooldown_ticks: codec::Bounded(5),
                delay_ticks: codec::Bounded(2),
                dismount_conditions: Some(KineticCondition {
                    max_duration_ticks: codec::Bounded(10),
                    min_speed: 0.0,
                    min_relative_speed: 0.0,
                }),
                knockback_conditions: Some(KineticCondition {
                    max_duration_ticks: codec::Bounded(20),
                    min_speed: 1.5,
                    min_relative_speed: 0.0,
                }),
                damage_conditions: Some(KineticCondition {
                    max_duration_ticks: codec::Bounded(30),
                    min_speed: 0.5,
                    min_relative_speed: 2.0,
                }),
                forward_movement: 0.5,
                damage_multiplier: 2.0,
                sound: Some(item_break()),
                hit_sound: Some(Holder::reference(ResourceLocation::minecraft(
                    "item.armor.equip_generic",
                ))),
            },
        ]
    }
}
