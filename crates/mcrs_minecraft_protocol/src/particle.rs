use mcrs_minecraft_registries::Wire;
use std::io::Write;

use mcrs_minecraft_core::codec;
use mcrs_minecraft_item::component::RgbInt;
use mcrs_minecraft_particle::keys::ParticleType;
use mcrs_minecraft_particle::{
    BlockParticle, ColorParticle, DustColorTransitionParticle, DustParticle, GeyserBaseParticle,
    GeyserParticle, ItemParticle, ParticleOptions, ParticleScale, PositionSource, PowerParticle,
    SculkChargeParticle, ShriekParticle, SpellParticle, TrailParticle, VibrationParticle,
};

use crate::item::wire::record_wire;
use crate::registry::static_registry_wire;
use crate::{Decode, Encode, VarInt};

macro_rules! particle_wire {
    ($($variant:ident $(($payload:ty))?),* $(,)?) => {
        fn encode_payload(options: &ParticleOptions<Wire>, w: impl Write) -> anyhow::Result<()> {
            match options {
                $(variant_pattern!(ParticleOptions::$variant, payload $(: $payload)?) => {
                    encode_arm!(payload, w $(, $payload)?)
                })*
            }
        }

        fn decode_payload(kind: ParticleType, r: &mut &[u8]) -> anyhow::Result<ParticleOptions<Wire>> {
            Ok(match kind {
                $(ParticleType::$variant => decode_arm!(ParticleOptions::$variant, r $(, $payload)?)),*
            })
        }
    };
}

macro_rules! variant_pattern {
    ($variant:path, $binding:ident) => {
        $variant
    };
    ($variant:path, $binding:ident : $payload:ty) => {
        $variant($binding)
    };
}

macro_rules! encode_arm {
    ($binding:ident, $w:ident) => {
        Ok(())
    };
    ($binding:ident, $w:ident, $payload:ty) => {
        $binding.encode($w)
    };
}

macro_rules! decode_arm {
    ($variant:path, $r:ident) => {
        $variant
    };
    ($variant:path, $r:ident, $payload:ty) => {
        $variant(Decode::decode($r)?)
    };
}

mcrs_minecraft_particle::for_each_particle_type!(particle_wire);

static_registry_wire!(ParticleType, "particle type");

impl Encode for ParticleOptions<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.kind().encode(&mut w)?;
        encode_payload(self, w)
    }
}

impl Decode<'_> for ParticleOptions<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let kind = ParticleType::decode(r)?;
        decode_payload(kind, r)
    }
}

record_wire!(BlockParticle<Wire> { block_state }, ItemParticle<Wire> { item });

impl Encode for GeyserParticle {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.water_blocks.0.encode(w)
    }
}

impl Decode<'_> for GeyserParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(GeyserParticle {
            water_blocks: codec::Bounded(i32::decode(r)?),
        })
    }
}

impl Encode for GeyserBaseParticle {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.water_blocks.0.encode(&mut w)?;
        self.burst_impulse_base.encode(w)
    }
}

impl Decode<'_> for GeyserBaseParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(GeyserBaseParticle {
            water_blocks: codec::Bounded(i32::decode(r)?),
            burst_impulse_base: f32::decode(r)?,
        })
    }
}

impl Encode for ParticleScale {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.0.encode(w)
    }
}

impl Decode<'_> for ParticleScale {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Self::clamped(f32::decode(r)?))
    }
}

record_wire!(
    PowerParticle { power },
    DustParticle { color, scale },
    DustColorTransitionParticle {
        from_color,
        to_color,
        scale
    },
    SpellParticle { color, power },
    ColorParticle { color },
    SculkChargeParticle { roll },
);

impl Encode for ShriekParticle {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.delay).encode(w)
    }
}

impl Decode<'_> for ShriekParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ShriekParticle {
            delay: VarInt::decode(r)?.0,
        })
    }
}

impl Encode for PositionSource {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            PositionSource::Block { pos } => {
                VarInt(0).encode(&mut w)?;
                pos.encode(w)
            }
            PositionSource::Entity {
                entity_id,
                y_offset,
            } => {
                VarInt(1).encode(&mut w)?;
                VarInt(*entity_id).encode(&mut w)?;
                y_offset.encode(w)
            }
        }
    }
}

impl Decode<'_> for PositionSource {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            0 => PositionSource::Block {
                pos: Decode::decode(r)?,
            },
            1 => PositionSource::Entity {
                entity_id: VarInt::decode(r)?.0,
                y_offset: f32::decode(r)?,
            },
            n => anyhow::bail!("unexpected enum discriminant {n} in `PositionSource`"),
        })
    }
}

impl Encode for VibrationParticle {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.destination.encode(&mut w)?;
        VarInt(self.arrival_in_ticks).encode(w)
    }
}

impl Decode<'_> for VibrationParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(VibrationParticle {
            destination: PositionSource::decode(r)?,
            arrival_in_ticks: VarInt::decode(r)?.0,
        })
    }
}

impl Encode for TrailParticle {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.target.encode(&mut w)?;
        self.color.encode(&mut w)?;
        VarInt(self.duration.0).encode(w)
    }
}

impl Decode<'_> for TrailParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(TrailParticle {
            target: <[f64; 3]>::decode(r)?,
            color: RgbInt::decode(r)?,
            duration: codec::Bounded(VarInt::decode(r)?.0),
        })
    }
}
