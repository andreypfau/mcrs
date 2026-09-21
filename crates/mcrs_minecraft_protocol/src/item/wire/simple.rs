use std::io::Write;

use mcrs_minecraft_core::codec::Bounded;

use crate::item::component::simple::*;
use crate::item::ctx::ctx_free;
use crate::item::wire::newtype_wire;
use crate::{Decode, Encode, VarInt};

impl Encode for UseEffects {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.can_sprint.encode(&mut w)?;
        self.interact_vibrations.encode(&mut w)?;
        self.speed_multiplier.encode(w)
    }
}

impl Decode<'_> for UseEffects {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(UseEffects {
            can_sprint: bool::decode(r)?,
            interact_vibrations: bool::decode(r)?,
            speed_multiplier: f32::decode(r)?,
        })
    }
}

ctx_free!(UseEffects);

impl Encode for CustomModelData {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.floats.encode(&mut w)?;
        self.flags.encode(&mut w)?;
        self.strings.encode(&mut w)?;
        self.colors.encode(w)
    }
}

impl Decode<'_> for CustomModelData {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(CustomModelData {
            floats: Vec::decode(r)?,
            flags: Vec::decode(r)?,
            strings: Vec::decode(r)?,
            colors: Vec::decode(r)?,
        })
    }
}

ctx_free!(CustomModelData);

impl Encode for TooltipDisplay {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
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
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
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

impl Encode for UseCooldown {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.seconds.encode(&mut w)?;
        self.cooldown_group.encode(w)
    }
}

impl Decode<'_> for UseCooldown {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(UseCooldown {
            seconds: f32::decode(r)?,
            cooldown_group: Option::decode(r)?,
        })
    }
}

ctx_free!(UseCooldown);

impl Encode for Weapon {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
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

impl Encode for AttackRange {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.min_reach.encode(&mut w)?;
        self.max_reach.encode(&mut w)?;
        self.min_creative_reach.encode(&mut w)?;
        self.max_creative_reach.encode(&mut w)?;
        self.hitbox_margin.encode(&mut w)?;
        self.mob_factor.encode(w)
    }
}

impl Decode<'_> for AttackRange {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(AttackRange {
            min_reach: f32::decode(r)?,
            max_reach: f32::decode(r)?,
            min_creative_reach: f32::decode(r)?,
            max_creative_reach: f32::decode(r)?,
            hitbox_margin: f32::decode(r)?,
            mob_factor: f32::decode(r)?,
        })
    }
}

ctx_free!(AttackRange);

newtype_wire!(BlockState);
