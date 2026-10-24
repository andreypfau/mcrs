use mcrs_minecraft_registries::Wire;
use std::io::Write;

use anyhow::bail;

use crate::item::component::consume::*;
use crate::item::keys::ConsumeEffectType;
use crate::item::wire::record_wire;
use crate::registry::{decode_registry_id, encode_registry_id};
use crate::{Decode, Encode};

impl Encode for ConsumeEffect<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(self.kind() as u16, &mut w)?;
        match self {
            Self::ApplyEffects {
                effects,
                probability,
            } => {
                effects.encode(&mut w)?;
                probability.encode(w)
            }
            Self::RemoveEffects { effects } => effects.encode(w),
            Self::ClearAllEffects => Ok(()),
            Self::TeleportRandomly {
                diameter,
                directional_particles,
            } => {
                diameter.encode(&mut w)?;
                directional_particles.encode(w)
            }
            Self::PlaySound { sound } => sound.encode(w),
        }
    }
}

impl Decode<'_> for ConsumeEffect<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = decode_registry_id(r)?;
        let Some(kind) = ConsumeEffectType::from_protocol_id(id) else {
            bail!("unknown consume effect type {id}");
        };
        Ok(match kind {
            ConsumeEffectType::ApplyEffects => Self::ApplyEffects {
                effects: Vec::decode(r)?,
                probability: f32::decode(r)?,
            },
            ConsumeEffectType::RemoveEffects => Self::RemoveEffects {
                effects: Decode::decode(r)?,
            },
            ConsumeEffectType::ClearAllEffects => Self::ClearAllEffects,
            ConsumeEffectType::TeleportRandomly => Self::TeleportRandomly {
                diameter: f32::decode(r)?,
                directional_particles: bool::decode(r)?,
            },
            ConsumeEffectType::PlaySound => Self::PlaySound {
                sound: Decode::decode(r)?,
            },
        })
    }
}

record_wire! {
    Consumable<Wire> { consume_seconds, animation, sound, has_consume_particles, on_consume_effects },
    DeathProtection<Wire> { death_effects },
}
