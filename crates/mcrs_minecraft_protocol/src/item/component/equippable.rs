use std::io::Write;

use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, FLOAT_ID, STRING_ID};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::entity::EquipmentSlot;
use crate::item::component::common::{EntityTypeReg, Holder};
use crate::item::component::sound::SoundEvent;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::{Decode, Encode, VarInt};

const SLOT_NAMES: [(&str, EquipmentSlot); 8] = [
    ("mainhand", EquipmentSlot::MainHand),
    ("offhand", EquipmentSlot::OffHand),
    ("feet", EquipmentSlot::Feet),
    ("legs", EquipmentSlot::Legs),
    ("chest", EquipmentSlot::Chest),
    ("head", EquipmentSlot::Head),
    ("body", EquipmentSlot::Body),
    ("saddle", EquipmentSlot::Saddle),
];

impl Serialize for EquipmentSlot {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(SLOT_NAMES[*self as usize].0)
    }
}

impl<'de> Deserialize<'de> for EquipmentSlot {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        SLOT_NAMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, slot)| *slot)
            .ok_or_else(|| D::Error::custom(format_args!("Unknown element name:{name}")))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Equippable {
    pub slot: EquipmentSlot,
    #[serde(default = "equip_generic", skip_serializing_if = "is_equip_generic")]
    pub equip_sound: Holder<SoundEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<ResourceLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_overlay: Option<ResourceLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_entities: Option<HolderSet<ResourceKey<EntityTypeReg>>>,
    #[serde(
        default = "default_true",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub dispensable: bool,
    #[serde(
        default = "default_true",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub swappable: bool,
    #[serde(
        default = "default_true",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub damage_on_hurt: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub equip_on_interact: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub can_be_sheared: bool,
    #[serde(default = "shears_snip", skip_serializing_if = "is_shears_snip")]
    pub shearing_sound: Holder<SoundEvent>,
}

pub const EQUIP_GENERIC_SOUND: &str = "item.armor.equip_generic";
pub const SHEARS_SNIP_SOUND: &str = "item.shears.snip";

fn equip_generic() -> Holder<SoundEvent> {
    Holder::reference(ResourceLocation::minecraft(EQUIP_GENERIC_SOUND))
}

fn is_equip_generic(sound: &Holder<SoundEvent>) -> bool {
    *sound == equip_generic()
}

fn shears_snip() -> Holder<SoundEvent> {
    Holder::reference(ResourceLocation::minecraft(SHEARS_SNIP_SOUND))
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

impl EncodeCtx for Equippable {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.slot.equippable_id() as i32).encode(&mut w)?;
        self.equip_sound.encode_ctx(ctx, &mut w)?;
        self.asset_id.encode(&mut w)?;
        self.camera_overlay.encode(&mut w)?;
        self.allowed_entities.encode_ctx(ctx, &mut w)?;
        self.dispensable.encode(&mut w)?;
        self.swappable.encode(&mut w)?;
        self.damage_on_hurt.encode(&mut w)?;
        self.equip_on_interact.encode(&mut w)?;
        self.can_be_sheared.encode(&mut w)?;
        self.shearing_sound.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Equippable {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Equippable {
            slot: u8::try_from(VarInt::decode(r)?.0)
                .map_or(EquipmentSlot::MainHand, EquipmentSlot::from_equippable_id),
            equip_sound: Holder::decode_ctx(ctx, r)?,
            asset_id: Option::decode(r)?,
            camera_overlay: Option::decode(r)?,
            allowed_entities: Option::decode_ctx(ctx, r)?,
            dispensable: bool::decode(r)?,
            swappable: bool::decode(r)?,
            damage_on_hurt: bool::decode(r)?,
            equip_on_interact: bool::decode(r)?,
            can_be_sheared: bool::decode(r)?,
            shearing_sound: Holder::decode_ctx(ctx, r)?,
        })
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
            Holder::Reference(key) if key.as_str() != "minecraft:item.shears.snip" => {
                tags.push(("shearing_sound", STRING_ID))
            }
            Holder::Reference(_) => {}
        }
        tags
    }

    fn samples() -> Vec<Self> {
        let key = |path: &str| ResourceKey::from_location(ResourceLocation::minecraft(path));
        vec![
            Equippable::new(EquipmentSlot::Head),
            Equippable {
                slot: EquipmentSlot::OffHand,
                equip_sound: Holder::reference(ResourceLocation::minecraft(
                    "item.armor.equip_iron",
                )),
                asset_id: Some(ResourceLocation::minecraft("iron")),
                camera_overlay: Some(ResourceLocation::minecraft("misc/pumpkinblur")),
                allowed_entities: Some(HolderSet::List(vec![key("zombie"), key("pig")])),
                dispensable: false,
                swappable: false,
                damage_on_hurt: false,
                equip_on_interact: true,
                can_be_sheared: true,
                shearing_sound: Holder::Direct(SoundEvent {
                    sound_id: ResourceLocation::new("mcrs", "snip"),
                    range: Some(3.5),
                }),
            },
            Equippable {
                allowed_entities: Some(HolderSet::Tag(ResourceLocation::minecraft("skeletons"))),
                ..Equippable::new(EquipmentSlot::Saddle)
            },
            Equippable {
                equip_sound: Holder::Direct(SoundEvent {
                    sound_id: ResourceLocation::new("mcrs", "equip"),
                    range: None,
                }),
                allowed_entities: Some(HolderSet::One(key("pig"))),
                ..Equippable::new(EquipmentSlot::Body)
            },
        ]
    }
}
