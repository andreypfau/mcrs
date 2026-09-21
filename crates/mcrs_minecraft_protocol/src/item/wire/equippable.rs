use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::entity::EquipmentSlot;
use crate::item::component::common::Holder;
use crate::item::component::equippable::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::{Decode, Encode, VarInt};

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
