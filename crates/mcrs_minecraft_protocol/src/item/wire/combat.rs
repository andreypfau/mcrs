use mcrs_minecraft_registries::Wire;
use std::io::Write;

use mcrs_minecraft_core::codec;

use crate::item::component::combat::*;
use crate::item::wire::record_wire;
use crate::{Decode, Encode, VarInt};

record_wire! { ItemDamageFunction { threshold, base, factor } }

record_wire! {
    BlocksAttacks<Wire> {
        block_delay_seconds,
        disable_cooldown_scale,
        damage_reductions,
        item_damage,
        bypassed_by,
        block_sound,
        disable_sound,
    },
    DamageReduction<Wire> { horizontal_blocking_angle, types, base, factor },
    PiercingWeapon<Wire> { deals_knockback, dismounts, sound, hit_sound },
}

impl Encode for KineticWeapon<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.contact_cooldown_ticks.0).encode(&mut w)?;
        VarInt(self.delay_ticks.0).encode(&mut w)?;
        self.dismount_conditions.encode(&mut w)?;
        self.knockback_conditions.encode(&mut w)?;
        self.damage_conditions.encode(&mut w)?;
        self.forward_movement.encode(&mut w)?;
        self.damage_multiplier.encode(&mut w)?;
        self.sound.encode(&mut w)?;
        self.hit_sound.encode(w)
    }
}

impl Decode<'_> for KineticWeapon<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(KineticWeapon {
            contact_cooldown_ticks: codec::Bounded(VarInt::decode(r)?.0),
            delay_ticks: codec::Bounded(VarInt::decode(r)?.0),
            dismount_conditions: Option::decode(r)?,
            knockback_conditions: Option::decode(r)?,
            damage_conditions: Option::decode(r)?,
            forward_movement: f32::decode(r)?,
            damage_multiplier: f32::decode(r)?,
            sound: Option::decode(r)?,
            hit_sound: Option::decode(r)?,
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
