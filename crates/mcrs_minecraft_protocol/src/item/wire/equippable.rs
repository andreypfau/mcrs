use mcrs_minecraft_registries::Wire;
use std::io::Write;

use crate::item::component::equippable::EquipmentSlot;
use crate::item::component::equippable::*;
use crate::{Decode, Encode, VarInt};

impl Encode for Equippable<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.slot.equippable_id() as i32).encode(&mut w)?;
        self.equip_sound.encode(&mut w)?;
        self.asset_id.encode(&mut w)?;
        self.camera_overlay.encode(&mut w)?;
        self.allowed_entities.encode(&mut w)?;
        self.dispensable.encode(&mut w)?;
        self.swappable.encode(&mut w)?;
        self.damage_on_hurt.encode(&mut w)?;
        self.equip_on_interact.encode(&mut w)?;
        self.can_be_sheared.encode(&mut w)?;
        self.shearing_sound.encode(w)
    }
}

impl Decode<'_> for Equippable<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Equippable {
            slot: u8::try_from(VarInt::decode(r)?.0)
                .map_or(EquipmentSlot::MainHand, EquipmentSlot::from_equippable_id),
            equip_sound: Decode::decode(r)?,
            asset_id: Option::decode(r)?,
            camera_overlay: Option::decode(r)?,
            allowed_entities: Option::decode(r)?,
            dispensable: bool::decode(r)?,
            swappable: bool::decode(r)?,
            damage_on_hurt: bool::decode(r)?,
            equip_on_interact: bool::decode(r)?,
            can_be_sheared: bool::decode(r)?,
            shearing_sound: Decode::decode(r)?,
        })
    }
}
