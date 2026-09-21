use std::io::Write;

use mcrs_minecraft_core::codec;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::combat::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
use crate::{Decode, Encode, VarInt};

impl EncodeCtx for BlocksAttacks {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.block_delay_seconds.encode(&mut w)?;
        self.disable_cooldown_scale.encode(&mut w)?;
        self.damage_reductions.encode_ctx(ctx, &mut w)?;
        self.item_damage.encode(&mut w)?;
        self.bypassed_by.encode_ctx(ctx, &mut w)?;
        self.block_sound.encode_ctx(ctx, &mut w)?;
        self.disable_sound.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for BlocksAttacks {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BlocksAttacks {
            block_delay_seconds: f32::decode(r)?,
            disable_cooldown_scale: f32::decode(r)?,
            damage_reductions: Vec::decode_ctx(ctx, r)?,
            item_damage: ItemDamageFunction::decode(r)?,
            bypassed_by: Option::decode_ctx(ctx, r)?,
            block_sound: Option::decode_ctx(ctx, r)?,
            disable_sound: Option::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for DamageReduction {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.horizontal_blocking_angle.encode(&mut w)?;
        self.types.encode_ctx(ctx, &mut w)?;
        self.base.encode(&mut w)?;
        self.factor.encode(w)
    }
}

impl DecodeCtx<'_> for DamageReduction {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(DamageReduction {
            horizontal_blocking_angle: f32::decode(r)?,
            types: Option::decode_ctx(ctx, r)?,
            base: f32::decode(r)?,
            factor: f32::decode(r)?,
        })
    }
}

impl Encode for ItemDamageFunction {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.threshold.encode(&mut w)?;
        self.base.encode(&mut w)?;
        self.factor.encode(w)
    }
}

impl Decode<'_> for ItemDamageFunction {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ItemDamageFunction {
            threshold: f32::decode(r)?,
            base: f32::decode(r)?,
            factor: f32::decode(r)?,
        })
    }
}

ctx_free!(ItemDamageFunction);

impl EncodeCtx for PiercingWeapon {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.deals_knockback.encode(&mut w)?;
        self.dismounts.encode(&mut w)?;
        self.sound.encode_ctx(ctx, &mut w)?;
        self.hit_sound.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for PiercingWeapon {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(PiercingWeapon {
            deals_knockback: bool::decode(r)?,
            dismounts: bool::decode(r)?,
            sound: Option::decode_ctx(ctx, r)?,
            hit_sound: Option::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for KineticWeapon {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.contact_cooldown_ticks.0).encode(&mut w)?;
        VarInt(self.delay_ticks.0).encode(&mut w)?;
        self.dismount_conditions.encode(&mut w)?;
        self.knockback_conditions.encode(&mut w)?;
        self.damage_conditions.encode(&mut w)?;
        self.forward_movement.encode(&mut w)?;
        self.damage_multiplier.encode(&mut w)?;
        self.sound.encode_ctx(ctx, &mut w)?;
        self.hit_sound.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for KineticWeapon {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(KineticWeapon {
            contact_cooldown_ticks: codec::Bounded(VarInt::decode(r)?.0),
            delay_ticks: codec::Bounded(VarInt::decode(r)?.0),
            dismount_conditions: Option::decode(r)?,
            knockback_conditions: Option::decode(r)?,
            damage_conditions: Option::decode(r)?,
            forward_movement: f32::decode(r)?,
            damage_multiplier: f32::decode(r)?,
            sound: Option::decode_ctx(ctx, r)?,
            hit_sound: Option::decode_ctx(ctx, r)?,
        })
    }
}

impl Encode for KineticCondition {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.max_duration_ticks.0).encode(&mut w)?;
        self.min_speed.encode(&mut w)?;
        self.min_relative_speed.encode(w)
    }
}

impl Decode<'_> for KineticCondition {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(KineticCondition {
            max_duration_ticks: codec::Bounded(VarInt::decode(r)?.0),
            min_speed: f32::decode(r)?,
            min_relative_speed: f32::decode(r)?,
        })
    }
}
