use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::LIST_ID;
use serde::{Deserialize, Serialize};

use crate::component::common::{AttributeReg, EquipmentSlotGroup, key, ordinal_enum};
use crate::component::registry_ref::null_as_default;
use crate::harness::Sample;
use mcrs_minecraft_text::IntoText;

use crate::Text;

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeModifiers(pub Vec<AttributeEntry>);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeEntry {
    #[serde(rename = "type")]
    pub attribute: ResourceKey<AttributeReg>,
    #[serde(flatten)]
    pub modifier: AttributeModifierValue,
    #[serde(
        default = "any_slot",
        deserialize_with = "slot_or_default",
        skip_serializing_if = "is_any"
    )]
    pub slot: EquipmentSlotGroup,
    #[serde(
        default,
        deserialize_with = "display_or_default",
        skip_serializing_if = "is_default"
    )]
    pub display: AttributeDisplay,
}

null_as_default! {
    slot_or_default: EquipmentSlotGroup = any_slot();
    display_or_default: AttributeDisplay = AttributeDisplay::Default;
}

fn any_slot() -> EquipmentSlotGroup {
    EquipmentSlotGroup::Any
}

fn is_any(slot: &EquipmentSlotGroup) -> bool {
    *slot == EquipmentSlotGroup::Any
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeModifierValue {
    pub id: ResourceLocation,
    pub amount: f64,
    pub operation: AttributeOperation,
}

ordinal_enum! {
    AttributeOperation { AddValue, AddMultipliedBase, AddMultipliedTotal }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttributeDisplay {
    #[default]
    Default,
    Hidden,
    Override {
        value: Text,
    },
}

impl AttributeDisplay {
    pub fn type_id(&self) -> i32 {
        match self {
            AttributeDisplay::Default => 0,
            AttributeDisplay::Hidden => 1,
            AttributeDisplay::Override { .. } => 2,
        }
    }
}

impl Sample for AttributeModifiers {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        let entry = |path: &str, id: &str, amount: f64, operation, slot, display| AttributeEntry {
            attribute: key(path),
            modifier: AttributeModifierValue {
                id: ResourceLocation::new("mcrs", id),
                amount,
                operation,
            },
            slot,
            display,
        };
        vec![
            AttributeModifiers::default(),
            AttributeModifiers(vec![entry(
                "attack_damage",
                "base_attack_damage",
                7.0,
                AttributeOperation::AddValue,
                EquipmentSlotGroup::Any,
                AttributeDisplay::Default,
            )]),
            AttributeModifiers(vec![
                entry(
                    "armor",
                    "armor",
                    -0.5,
                    AttributeOperation::AddMultipliedTotal,
                    EquipmentSlotGroup::Head,
                    AttributeDisplay::Hidden,
                ),
                entry(
                    "attack_damage",
                    "dmg",
                    1.5,
                    AttributeOperation::AddMultipliedBase,
                    EquipmentSlotGroup::Saddle,
                    AttributeDisplay::Override {
                        value: Text::text("shown"),
                    },
                ),
                entry(
                    "armor",
                    "styled",
                    2.0,
                    AttributeOperation::AddValue,
                    EquipmentSlotGroup::Armor,
                    AttributeDisplay::Override {
                        value: "styled".bold(),
                    },
                ),
            ]),
        ]
    }
}
