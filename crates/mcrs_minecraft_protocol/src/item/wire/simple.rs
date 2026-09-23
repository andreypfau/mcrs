use mcrs_minecraft_core::codec::Bounded;

use crate::item::component::simple::*;
use crate::item::ctx::ctx_free;
use crate::item::wire::{newtype_wire, record_wire};
use crate::{Decode, Encode, VarInt};

record_wire! {
    UseEffects { can_sprint, interact_vibrations, speed_multiplier },
    CustomModelData { floats, flags, strings, colors },
    UseCooldown { seconds, cooldown_group },
    AttackRange {
        min_reach,
        max_reach,
        min_creative_reach,
        max_creative_reach,
        hitbox_margin,
        mob_factor,
    },
}

impl Encode for TooltipDisplay {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        self.hide_tooltip.encode(&mut w)?;
        self.hidden_components.encode(w)
    }
}

impl Decode<'_> for TooltipDisplay {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(TooltipDisplay::new(bool::decode(r)?, Vec::decode(r)?))
    }
}

ctx_free!(TooltipDisplay);

impl Encode for Food {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        VarInt(self.nutrition.0).encode(&mut w)?;
        self.saturation.encode(&mut w)?;
        self.can_always_eat.encode(w)
    }
}

impl Decode<'_> for Food {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Food {
            nutrition: Bounded(VarInt::decode(r)?.0),
            saturation: f32::decode(r)?,
            can_always_eat: bool::decode(r)?,
        })
    }
}

ctx_free!(Food);

impl Encode for Weapon {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        VarInt(self.item_damage_per_attack.0).encode(&mut w)?;
        self.disable_blocking_for_seconds.encode(w)
    }
}

impl Decode<'_> for Weapon {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Weapon {
            item_damage_per_attack: Bounded(VarInt::decode(r)?.0),
            disable_blocking_for_seconds: f32::decode(r)?,
        })
    }
}

ctx_free!(Weapon);

newtype_wire!(BlockState);
