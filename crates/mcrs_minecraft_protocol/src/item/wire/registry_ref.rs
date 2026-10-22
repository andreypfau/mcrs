use std::io::Write;

use crate::item::ctx::WireCtx;
use anyhow::{bail, ensure};
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registries::Namer;

use crate::item::component::registry_ref::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::{newtype_ctx_wire, record_ctx_wire};
use crate::{Decode, Encode, VarInt};

newtype_ctx_wire!(
    DamageTypeRef,
    BlockTransformerRef,
    StoredEnchantments,
    ProvidesBannerPatterns,
    SuspiciousStewEffects,
);
record_ctx_wire! {
    DamageResistant { types },
    ToolRule { blocks, speed, correct_for_drops },
    Repairable { items },
    MobVisibility { targeting_entity_types, visibility },
}

impl EncodeCtx for Enchantments {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0.len() as i32).encode(&mut w)?;
        for (enchantment, level) in &self.0 {
            enchantment.encode_ctx(ctx, &mut w)?;
            VarInt(*level).encode(&mut w)?;
        }
        Ok(())
    }
}

impl DecodeCtx<'_> for Enchantments {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode a map with negative length");
        let mut entries: Vec<(Id<EnchantmentData>, i32)> =
            Vec::with_capacity((len as usize).min(r.len()));
        for _ in 0..len {
            let enchantment = Id::decode_ctx(ctx, r)?;
            let level = VarInt::decode(r)?.0;
            if !(0..=MAX_ENCHANTMENT_LEVEL).contains(&level) {
                let name = ctx
                    .registries()
                    .ok()
                    .and_then(|set| Namer::name(set, enchantment))
                    .map_or_else(|| format!("{enchantment:?}"), ToString::to_string);
                bail!("Enchantment {name} has invalid level {level}");
            }
            match entries.iter_mut().find(|(k, _)| *k == enchantment) {
                Some(entry) => entry.1 = level,
                None => entries.push((enchantment, level)),
            }
        }
        Ok(Enchantments(entries))
    }
}

impl EncodeCtx for Tool {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        self.rules.encode_ctx(ctx, &mut w)?;
        self.default_mining_speed.encode(&mut w)?;
        VarInt(self.damage_per_block.0).encode(&mut w)?;
        self.can_destroy_blocks_in_creative.encode(w)
    }
}

impl DecodeCtx<'_> for Tool {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Tool {
            rules: Vec::decode_ctx(ctx, r)?,
            default_mining_speed: f32::decode(r)?,
            damage_per_block: Bounded(VarInt::decode(r)?.0),
            can_destroy_blocks_in_creative: bool::decode(r)?,
        })
    }
}

impl EncodeCtx for StewEntry {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode_ctx(ctx, &mut w)?;
        VarInt(self.duration).encode(w)
    }
}

impl DecodeCtx<'_> for StewEntry {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(StewEntry {
            id: DecodeCtx::decode_ctx(ctx, r)?,
            duration: VarInt::decode(r)?.0,
        })
    }
}
