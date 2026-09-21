use std::io::Write;

use anyhow::bail;
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::common::{Holder, ItemUseAnimation};
use crate::item::component::consume::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::{Decode, Encode, VarInt};

impl EncodeCtx for ConsumeEffect {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.kind() as i32).encode(&mut w)?;
        match self {
            Self::ApplyEffects {
                effects,
                probability,
            } => {
                effects.encode_ctx(ctx, &mut w)?;
                probability.encode(w)
            }
            Self::RemoveEffects { effects } => effects.encode_ctx(ctx, w),
            Self::ClearAllEffects => Ok(()),
            Self::TeleportRandomly {
                diameter,
                directional_particles,
            } => {
                diameter.encode(&mut w)?;
                directional_particles.encode(w)
            }
            Self::PlaySound { sound } => sound.encode_ctx(ctx, w),
        }
    }
}

impl DecodeCtx<'_> for ConsumeEffect {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let Some(kind) = ConsumeEffectType::from_wire_id(id) else {
            bail!("unknown consume effect type {id}");
        };
        Ok(match kind {
            ConsumeEffectType::ApplyEffects => Self::ApplyEffects {
                effects: Vec::decode_ctx(ctx, r)?,
                probability: f32::decode(r)?,
            },
            ConsumeEffectType::RemoveEffects => Self::RemoveEffects {
                effects: HolderSet::decode_ctx(ctx, r)?,
            },
            ConsumeEffectType::ClearAllEffects => Self::ClearAllEffects,
            ConsumeEffectType::TeleportRandomly => Self::TeleportRandomly {
                diameter: f32::decode(r)?,
                directional_particles: bool::decode(r)?,
            },
            ConsumeEffectType::PlaySound => Self::PlaySound {
                sound: Holder::decode_ctx(ctx, r)?,
            },
        })
    }
}

impl EncodeCtx for Consumable {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.consume_seconds.encode(&mut w)?;
        self.animation.encode(&mut w)?;
        self.sound.encode_ctx(ctx, &mut w)?;
        self.has_consume_particles.encode(&mut w)?;
        self.on_consume_effects.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Consumable {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Consumable {
            consume_seconds: f32::decode(r)?,
            animation: ItemUseAnimation::decode(r)?,
            sound: Holder::decode_ctx(ctx, r)?,
            has_consume_particles: bool::decode(r)?,
            on_consume_effects: Vec::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for DeathProtection {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.death_effects.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for DeathProtection {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(DeathProtection {
            death_effects: Vec::decode_ctx(ctx, r)?,
        })
    }
}
