use std::io::Write;

use anyhow::{bail, ensure};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::DeserializeSeed;

use crate::item::component::common::CompactList;
use crate::item::component::predicate::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, decode_nbt_wire, encode_nbt_wire};
use crate::item::kind::ItemComponentKind;
use crate::item::patch::ComponentMap;
use crate::item::wire::newtype_ctx_wire;
use crate::{Decode, Encode, VarInt};

newtype_ctx_wire!(CanPlaceOn, CanBreak, Lock);

impl EncodeCtx for AdventureModePredicate {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for AdventureModePredicate {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        CompactList::decode_ctx(ctx, r).map(AdventureModePredicate)
    }
}

impl EncodeCtx for BlockPredicate {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.blocks.encode_ctx(ctx, &mut w)?;
        self.state.encode_ctx(ctx, &mut w)?;
        self.nbt.encode_ctx(ctx, &mut w)?;
        self.matchers.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for BlockPredicate {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BlockPredicate {
            blocks: Option::decode_ctx(ctx, r)?,
            state: Option::decode_ctx(ctx, r)?,
            nbt: Option::decode_ctx(ctx, r)?,
            matchers: DataComponentMatchers::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for ItemPredicate {
    fn encode_ctx(&self, _: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        encode_nbt_wire(self, w)
    }
}

impl DecodeCtx<'_> for ItemPredicate {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_nbt_wire(r)
    }
}

impl EncodeCtx for StatePropertiesPredicate {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for StatePropertiesPredicate {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(StatePropertiesPredicate)
    }
}

/// The writer is erased before recursing into the component dispatch, as
/// `ComponentPatch` does: a generic `impl Write` would otherwise gain one
/// `&mut` per level of a value that contains a value of its own kind and
/// never reach a fixed type.
impl EncodeCtx for DataComponentMatchers {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
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
                    encode_nbt_wire(predicate, &mut *w)?;
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
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
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
                    let kind = ComponentPredicateType::decode(r)?;
                    ComponentPredicateEntry::Typed(decode_predicate_wire(kind, r)?)
                }
                false => {
                    let kind = ItemComponentKind::decode(r)?;
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

impl Encode for ComponentPredicateType {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(*self as i32).encode(w)
    }
}

impl Decode<'_> for ComponentPredicateType {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        match usize::try_from(id).ok().and_then(|id| Self::ALL.get(id)) {
            Some(kind) => Ok(*kind),
            None => bail!("unknown data component predicate type {id}"),
        }
    }
}

/// One network NBT tag of whatever root type the predicate's codec writes,
/// read back through that codec.
fn decode_predicate_wire(
    kind: ComponentPredicateType,
    r: &mut &[u8],
) -> anyhow::Result<ComponentPredicate> {
    match r.first() {
        None => bail!("empty input for a network NBT tag"),
        Some(&mcrs_minecraft_nbt::END_ID) => bail!("a network NBT tag must not be TAG_End"),
        Some(_) => {}
    }
    let mut cursor = std::io::Cursor::new(*r);
    let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
    let value = kind.deserialize(&mut d)?;
    *r = &r[cursor.position() as usize..];
    Ok(value)
}
