use std::io::Write;

use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::LIST_ID;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{AttributeReg, EquipmentSlotGroup, ordinal_enum};
use crate::item::component::registry_ref::null_as_default;
use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
use crate::item::harness::Sample;
use crate::text::{IntoText, Text};
use crate::{Decode, Encode, VarInt};

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
        skip_serializing_if = "is_default_display"
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

fn is_default_display(display: &AttributeDisplay) -> bool {
    *display == AttributeDisplay::Default
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct AttributeModifierValue {
    pub id: ResourceLocation,
    pub amount: f64,
    pub operation: AttributeOperation,
}

ctx_free!(AttributeModifierValue);

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
    fn type_id(&self) -> i32 {
        match self {
            AttributeDisplay::Default => 0,
            AttributeDisplay::Hidden => 1,
            AttributeDisplay::Override { .. } => 2,
        }
    }
}

impl EncodeCtx for AttributeDisplay {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.type_id()).encode(&mut w)?;
        match self {
            AttributeDisplay::Override { value } => value.encode(w),
            _ => Ok(()),
        }
    }
}

/// An unknown type id reads as `Default`, `ByIdMap` `ZERO`.
impl DecodeCtx<'_> for AttributeDisplay {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            1 => AttributeDisplay::Hidden,
            2 => AttributeDisplay::Override {
                value: Text::decode(r)?,
            },
            _ => AttributeDisplay::Default,
        })
    }
}

impl EncodeCtx for AttributeEntry {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.attribute.encode_ctx(ctx, &mut w)?;
        self.modifier.encode(&mut w)?;
        self.slot.encode(&mut w)?;
        self.display.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for AttributeEntry {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(AttributeEntry {
            attribute: ResourceKey::decode_ctx(ctx, r)?,
            modifier: AttributeModifierValue::decode(r)?,
            slot: EquipmentSlotGroup::decode(r)?,
            display: AttributeDisplay::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for AttributeModifiers {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for AttributeModifiers {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(AttributeModifiers)
    }
}

impl Sample for AttributeModifiers {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        let attribute = |path: &str| ResourceKey::from_location(ResourceLocation::minecraft(path));
        let entry = |path: &str, id: &str, amount: f64, operation, slot, display| AttributeEntry {
            attribute: attribute(path),
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
