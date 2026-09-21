use std::io::Write;

use mcrs_minecraft_core::codec;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::combat::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
use crate::item::wire::record_ctx_wire;
use crate::{Decode, Encode, VarInt};

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

record_ctx_wire! {
    BlocksAttacks {
        block_delay_seconds,
        disable_cooldown_scale,
        damage_reductions,
        item_damage,
        bypassed_by,
        block_sound,
        disable_sound,
    },
    DamageReduction { horizontal_blocking_angle, types, base, factor },
    PiercingWeapon { deals_knockback, dismounts, sound, hit_sound },
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
