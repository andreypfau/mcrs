use std::io::Write;

use crate::item::ctx::WireCtx;
use anyhow::ensure;
use mcrs_minecraft_item::keys::DataComponentPredicateType;
use mcrs_minecraft_registries::{Named, Resolve};
use serde::de::DeserializeSeed;

use crate::item::component::predicate::*;
use crate::item::ctx::{
    DecodeCtx, EncodeCtx, decode_named_nbt, decode_nbt_wire, encode_named_nbt, encode_nbt_wire,
    read_nbt_wire, registries,
};
use crate::item::patch::ComponentMap;
use crate::item::wire::newtype_ctx_wire;
use crate::registry::static_registry_wire;
use crate::{Decode, Encode, VarInt};
use mcrs_minecraft_item::keys::DataComponentType;

newtype_ctx_wire!(
    CanPlaceOn,
    CanBreak,
    Lock,
    AdventureModePredicate,
    StatePropertiesPredicate,
);
impl EncodeCtx for ItemPredicate {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        encode_named_nbt(self, ctx, w)
    }
}

impl DecodeCtx<'_> for ItemPredicate {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_named_nbt(ctx, r)
    }
}
static_registry_wire!(DataComponentPredicateType, "data component predicate type");

impl EncodeCtx for BlockPredicate {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        self.blocks.encode_ctx(ctx, &mut w)?;
        self.state.encode_ctx(ctx, &mut w)?;
        self.nbt.encode_ctx(ctx, &mut w)?;
        self.matchers.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for BlockPredicate {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BlockPredicate {
            blocks: Option::decode_ctx(ctx, r)?,
            state: Option::decode_ctx(ctx, r)?,
            nbt: Option::decode_ctx(ctx, r)?,
            matchers: DataComponentMatchers::decode_ctx(ctx, r)?,
        })
    }
}

/// The writer is erased before recursing into the component dispatch, as
/// `ComponentPatch` does: a generic `impl Write` would otherwise gain one
/// `&mut` per level of a value that contains a value of its own kind and
/// never reach a fixed type.
impl EncodeCtx for DataComponentMatchers {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        let w: &mut dyn Write = &mut w;
        self.components.encode_ctx(ctx, &mut *w)?;
        let predicates = &self.predicates.0;
        ensure!(
            predicates.len() <= MAX_PARTIAL_PREDICATES,
            "list of {} entries exceeds the maximum of {MAX_PARTIAL_PREDICATES}",
            predicates.len()
        );
        VarInt(predicates.len() as i32).encode(&mut *w)?;
        for entry in predicates {
            match entry {
                ComponentPredicateEntry::Typed(predicate) => {
                    true.encode(&mut *w)?;
                    predicate.kind().encode(&mut *w)?;
                    encode_nbt_wire(&predicate.name(registries(ctx)?)?, &mut *w)?;
                }
                ComponentPredicateEntry::AnyValue(kind) => {
                    false.encode(&mut *w)?;
                    kind.encode(&mut *w)?;
                    encode_nbt_wire(&UnitMap, &mut *w)?;
                }
            }
        }
        Ok(())
    }
}

impl DecodeCtx<'_> for DataComponentMatchers {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        let components = ComponentMap::decode_ctx(ctx, r)?;
        let partial = VarInt::decode(r)?.0;
        ensure!(
            (0..=MAX_PARTIAL_PREDICATES as i32).contains(&partial),
            "list of {partial} entries exceeds the maximum of {MAX_PARTIAL_PREDICATES}"
        );
        let mut predicates = ComponentPredicates::default();
        for _ in 0..partial {
            let entry = match bool::decode(r)? {
                true => {
                    let kind = DataComponentPredicateType::decode(r)?;
                    let named = decode_predicate_wire(kind, r)?;
                    if ctx.walking() {
                        continue;
                    }
                    ComponentPredicateEntry::Typed(ComponentPredicate::resolve(
                        named,
                        registries(ctx)?,
                    )?)
                }
                false => {
                    let kind = DataComponentType::decode(r)?;
                    decode_nbt_wire::<UnitMap>(r)?;
                    ComponentPredicateEntry::AnyValue(kind)
                }
            };
            ensure!(
                !predicates.0.iter().any(|seen| seen.same_key(&entry)),
                "duplicate data component predicate"
            );
            predicates.0.push(entry);
        }
        Ok(DataComponentMatchers {
            components,
            predicates,
        })
    }
}

/// One network NBT tag of whatever root type the predicate's codec writes,
/// read back through that codec.
fn decode_predicate_wire(
    kind: DataComponentPredicateType,
    r: &mut &[u8],
) -> anyhow::Result<ComponentPredicate<Named>> {
    read_nbt_wire(r, |d| Ok(kind.deserialize(&mut *d)?))
}
