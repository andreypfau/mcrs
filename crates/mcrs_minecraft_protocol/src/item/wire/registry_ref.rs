use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registries::{Wire, WireId};

use crate::item::component::registry_ref::*;
use crate::item::wire::{newtype_wire, record_wire};
use crate::{Decode, Encode, VarInt};

newtype_wire!(
    DamageTypeRef<Wire>,
    BlockTransformerRef<Wire>,
    StoredEnchantments<Wire>,
    ProvidesBannerPatterns<Wire>,
    SuspiciousStewEffects<Wire>
);
record_wire! {
    DamageResistant<Wire> { types },
    ToolRule<Wire> { blocks, speed, correct_for_drops },
    Repairable<Wire> { items },
    MobVisibility<Wire> { targeting_entity_types, visibility },
}

impl Encode for Enchantments<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0.len() as i32).encode(&mut w)?;
        for (enchantment, level) in &self.0 {
            enchantment.encode(&mut w)?;
            VarInt(*level).encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for Enchantments<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode a map with negative length");
        let mut entries: Vec<(WireId<EnchantmentData>, i32)> =
            Vec::with_capacity((len as usize).min(r.len()));
        for _ in 0..len {
            let enchantment = WireId::decode(r)?;
            let level = VarInt::decode(r)?.0;
            ensure!(
                (0..=MAX_ENCHANTMENT_LEVEL).contains(&level),
                "Enchantment {enchantment:?} has invalid level {level}"
            );
            match entries.iter_mut().find(|(k, _)| *k == enchantment) {
                Some(entry) => entry.1 = level,
                None => entries.push((enchantment, level)),
            }
        }
        Ok(Enchantments(entries))
    }
}

impl Encode for Tool<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.rules.encode(&mut w)?;
        self.default_mining_speed.encode(&mut w)?;
        VarInt(self.damage_per_block.0).encode(&mut w)?;
        self.can_destroy_blocks_in_creative.encode(w)
    }
}

impl Decode<'_> for Tool<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Tool {
            rules: Vec::decode(r)?,
            default_mining_speed: f32::decode(r)?,
            damage_per_block: Bounded(VarInt::decode(r)?.0),
            can_destroy_blocks_in_creative: bool::decode(r)?,
        })
    }
}

impl Encode for StewEntry<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode(&mut w)?;
        VarInt(self.duration).encode(w)
    }
}

impl Decode<'_> for StewEntry<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(StewEntry {
            id: Decode::decode(r)?,
            duration: VarInt::decode(r)?.0,
        })
    }
}
