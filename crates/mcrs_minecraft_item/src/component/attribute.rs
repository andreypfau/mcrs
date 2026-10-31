use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_nbt::LIST_ID;
use mcrs_minecraft_registries::{Refs, Resolved, registry_value};
use mcrs_minecraft_resource::{ResourceLocation, StaticResourceLocation, rl};
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::common::{EquipmentSlotGroup, ordinal_enum};
use crate::component::registry_ref::null_as_default;
use crate::harness::{Sample, entry};
use mcrs_minecraft_entity::keys::Attribute;
use mcrs_minecraft_text::IntoText;

use crate::Text;
use crate::wire::ordinal_enum_wire;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::{Decode, Encode, Result, VarInt};
use std::io::Write;

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
#[serde(transparent)]
pub struct AttributeModifiers<M: Refs = Resolved>(pub Vec<AttributeEntry<M>>);

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
pub struct AttributeEntry<M: Refs = Resolved> {
    #[serde(rename = "type")]
    pub attribute: M::Id<Attribute>,
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
        skip_serializing_if = "is_default",
        bound(deserialize = "AttributeDisplay<M>: Deserialize<'de>")
    )]
    pub display: AttributeDisplay<M>,
}

null_as_default! {
    slot_or_default: EquipmentSlotGroup = any_slot();
}

fn display_or_default<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(
    d: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

fn any_slot() -> EquipmentSlotGroup {
    EquipmentSlotGroup::Any
}

fn is_any(slot: &EquipmentSlotGroup) -> bool {
    *slot == EquipmentSlotGroup::Any
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct AttributeModifierValue {
    pub id: ResourceLocation,
    pub amount: f64,
    pub operation: AttributeOperation,
}

ordinal_enum! {
    AttributeOperation { AddValue, AddMultipliedBase, AddMultipliedTotal }
}

#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Default, Serialize, Deserialize,
)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttributeDisplay<M: Refs = Resolved> {
    #[default]
    Default,
    Hidden,
    Override {
        value: Text<M::Nbt>,
    },
}

impl<M: Refs> AttributeDisplay<M> {
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
        let entry =
            |path: &str, id: StaticResourceLocation, amount: f64, operation, slot, display| {
                AttributeEntry {
                    attribute: entry(path),
                    modifier: AttributeModifierValue {
                        id: id.to_arc(),
                        amount,
                        operation,
                    },
                    slot,
                    display,
                }
            };
        vec![
            AttributeModifiers::default(),
            AttributeModifiers(vec![entry(
                "attack_damage",
                rl!("mcrs:base_attack_damage"),
                7.0,
                AttributeOperation::AddValue,
                EquipmentSlotGroup::Any,
                AttributeDisplay::Default,
            )]),
            AttributeModifiers(vec![
                entry(
                    "armor",
                    rl!("mcrs:armor"),
                    -0.5,
                    AttributeOperation::AddMultipliedTotal,
                    EquipmentSlotGroup::Head,
                    AttributeDisplay::Hidden,
                ),
                entry(
                    "attack_damage",
                    rl!("mcrs:dmg"),
                    1.5,
                    AttributeOperation::AddMultipliedBase,
                    EquipmentSlotGroup::Saddle,
                    AttributeDisplay::Override {
                        value: Text::text("shown"),
                    },
                ),
                entry(
                    "armor",
                    rl!("mcrs:styled"),
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

ordinal_enum_wire!(AttributeOperation);

impl Encode for AttributeDisplay<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        VarInt(self.type_id()).encode(&mut w)?;
        match self {
            AttributeDisplay::Override { value } => value.encode(w),
            _ => Ok(()),
        }
    }
}

/// An unknown type id reads as `Default`.
impl Decode<'_> for AttributeDisplay<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            1 => AttributeDisplay::Hidden,
            2 => AttributeDisplay::Override {
                value: Decode::decode(r)?,
            },
            _ => AttributeDisplay::Default,
        })
    }
}
