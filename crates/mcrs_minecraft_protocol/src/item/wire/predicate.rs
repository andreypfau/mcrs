use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_item::keys::DataComponentPredicateType;
use mcrs_minecraft_registries::{Named, Wire};
use serde::de::DeserializeSeed;

use crate::item::component::common::ValueMatcher;
use crate::item::component::predicate::*;
use crate::item::lookup::{decode_nbt_wire, encode_nbt_wire, nested, read_nbt_wire};
use crate::item::patch::ComponentMap;
use crate::item::wire::newtype_wire;
use crate::registry::static_registry_wire;
use crate::{Decode, Encode, VarInt};
use mcrs_minecraft_item::keys::DataComponentType;

newtype_wire!(
    CanPlaceOn<Wire>,
    CanBreak<Wire>,
    AdventureModePredicate<Wire>
);
impl Encode for StatePropertiesPredicate {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.0.encode(w)
    }
}

impl Decode<'_> for StatePropertiesPredicate {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let properties: Vec<(String, ValueMatcher)> = Vec::decode(r)?;
        for (at, (name, _)) in properties.iter().enumerate() {
            ensure!(
                properties[..at].iter().all(|(seen, _)| seen != name),
                "duplicate key in a wire map"
            );
        }
        Ok(StatePropertiesPredicate(properties))
    }
}
static_registry_wire!(DataComponentPredicateType, "data component predicate type");

impl Encode for BlockPredicate<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.blocks.encode(&mut w)?;
        self.state.encode(&mut w)?;
        self.nbt.encode(&mut w)?;
        self.matchers.encode(w)
    }
}

/// A matcher map holds components that hold block predicates again.
impl Decode<'_> for BlockPredicate<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        nested(|| {
            Ok(BlockPredicate {
                blocks: Option::decode(r)?,
                state: Option::decode(r)?,
                nbt: Option::decode(r)?,
                matchers: DataComponentMatchers::decode(r)?,
            })
        })
    }
}

/// The writer is erased before recursing into the component dispatch, as
/// `ComponentPatch` does: a generic `impl Write` would otherwise gain one
/// `&mut` per level of a value that contains a value of its own kind and
/// never reach a fixed type.
impl Encode for DataComponentMatchers<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let w: &mut dyn Write = &mut w;
        self.components.encode(&mut *w)?;
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

impl Decode<'_> for DataComponentMatchers<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let components = ComponentMap::decode(r)?;
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
                    ComponentPredicateEntry::Typed(decode_predicate_wire(kind, r)?)
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
