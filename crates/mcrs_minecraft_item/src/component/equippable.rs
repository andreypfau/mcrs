use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_keys::{EntityType, sound_event};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, FLOAT_ID, STRING_ID};
use mcrs_minecraft_registry::HolderSet;
use serde::{Deserialize, Serialize};

use crate::component::common::{Holder, list_set, one_set, serialize_optional_set, tag_set};
use crate::component::registry_ref::null_as_default;
use crate::component::sound::SoundEvent;
use crate::harness::Sample;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Feet,
    Legs,
    Chest,
    Head,
    Body,
    Saddle,
}

impl EquipmentSlot {
    pub const ALL: [Self; 8] = [
        Self::MainHand,
        Self::OffHand,
        Self::Feet,
        Self::Legs,
        Self::Chest,
        Self::Head,
        Self::Body,
        Self::Saddle,
    ];

    pub fn from_id(id: u8) -> anyhow::Result<Self> {
        Self::ALL
            .get(id as usize)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("invalid equipment slot {id}"))
    }

    /// `EquipmentSlot.STREAM_CODEC` numbers the slots differently from the
    /// equipment packet's ordinal: hands first, then armour, offhand at 5.
    pub const fn equippable_id(self) -> u8 {
        match self {
            Self::MainHand => 0,
            Self::Feet => 1,
            Self::Legs => 2,
            Self::Chest => 3,
            Self::Head => 4,
            Self::OffHand => 5,
            Self::Body => 6,
            Self::Saddle => 7,
        }
    }

    /// Out of range reads as the first slot (`ByIdMap` `ZERO`).
    pub const fn from_equippable_id(id: u8) -> Self {
        match id {
            1 => Self::Feet,
            2 => Self::Legs,
            3 => Self::Chest,
            4 => Self::Head,
            5 => Self::OffHand,
            6 => Self::Body,
            7 => Self::Saddle,
            _ => Self::MainHand,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Equippable {
    pub slot: EquipmentSlot,
    #[serde(
        default = "equip_generic",
        deserialize_with = "equip_sound_or_default",
        skip_serializing_if = "is_equip_generic"
    )]
    pub equip_sound: Holder<SoundEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<ResourceLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_overlay: Option<ResourceLocation>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_set"
    )]
    pub allowed_entities: Option<HolderSet<EntityType>>,
    #[serde(
        default = "default_true",
        deserialize_with = "true_or_default",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub dispensable: bool,
    #[serde(
        default = "default_true",
        deserialize_with = "true_or_default",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub swappable: bool,
    #[serde(
        default = "default_true",
        deserialize_with = "true_or_default",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub damage_on_hurt: bool,
    #[serde(
        default,
        deserialize_with = "false_or_default",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub equip_on_interact: bool,
    #[serde(
        default,
        deserialize_with = "false_or_default",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub can_be_sheared: bool,
    #[serde(
        default = "shears_snip",
        deserialize_with = "shearing_sound_or_default",
        skip_serializing_if = "is_shears_snip"
    )]
    pub shearing_sound: Holder<SoundEvent>,
}

null_as_default! {
    equip_sound_or_default: Holder<SoundEvent> = equip_generic();
    shearing_sound_or_default: Holder<SoundEvent> = shears_snip();
    true_or_default: bool = true;
    false_or_default: bool = false;
}

fn equip_generic() -> Holder<SoundEvent> {
    Holder::Reference(sound_event::ITEM_ARMOR_EQUIP_GENERIC)
}

fn is_equip_generic(sound: &Holder<SoundEvent>) -> bool {
    *sound == equip_generic()
}

fn shears_snip() -> Holder<SoundEvent> {
    Holder::Reference(sound_event::ITEM_SHEARS_SNIP)
}

fn is_shears_snip(sound: &Holder<SoundEvent>) -> bool {
    *sound == shears_snip()
}

impl Equippable {
    pub fn new(slot: EquipmentSlot) -> Self {
        Equippable {
            slot,
            equip_sound: equip_generic(),
            asset_id: None,
            camera_overlay: None,
            allowed_entities: None,
            dispensable: true,
            swappable: true,
            damage_on_hurt: true,
            equip_on_interact: false,
            can_be_sheared: false,
            shearing_sound: shears_snip(),
        }
    }
}

impl Sample for Equippable {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID), ("slot", STRING_ID)];
        if self.asset_id.is_some() {
            tags.push(("asset_id", STRING_ID));
        }
        if !self.dispensable {
            tags.push(("dispensable", BYTE_ID));
        }
        if self.can_be_sheared {
            tags.push(("can_be_sheared", BYTE_ID));
        }
        match &self.shearing_sound {
            Holder::Direct(SoundEvent { range: Some(_), .. }) => tags.extend([
                ("shearing_sound", COMPOUND_ID),
                ("shearing_sound.sound_id", STRING_ID),
                ("shearing_sound.range", FLOAT_ID),
            ]),
            Holder::Direct(_) => tags.push(("shearing_sound", COMPOUND_ID)),
            Holder::Reference(id) if *id != sound_event::ITEM_SHEARS_SNIP => {
                tags.push(("shearing_sound", STRING_ID))
            }
            Holder::Reference(_) => {}
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Equippable::new(EquipmentSlot::Head),
            Equippable {
                slot: EquipmentSlot::OffHand,
                equip_sound: Holder::Reference(sound_event::ITEM_ARMOR_EQUIP_IRON),
                asset_id: Some(rl!("minecraft:iron").to_arc()),
                camera_overlay: Some(rl!("minecraft:misc/pumpkinblur").to_arc()),
                allowed_entities: Some(list_set(&["zombie", "pig"])),
                dispensable: false,
                swappable: false,
                damage_on_hurt: false,
                equip_on_interact: true,
                can_be_sheared: true,
                shearing_sound: Holder::Direct(SoundEvent {
                    sound_id: rl!("mcrs:snip").to_arc(),
                    range: Some(3.5),
                }),
            },
            Equippable {
                allowed_entities: Some(tag_set("skeletons")),
                ..Equippable::new(EquipmentSlot::Saddle)
            },
            Equippable {
                equip_sound: Holder::Direct(SoundEvent {
                    sound_id: rl!("mcrs:equip").to_arc(),
                    range: None,
                }),
                allowed_entities: Some(one_set("pig")),
                ..Equippable::new(EquipmentSlot::Body)
            },
        ]
    }
}
