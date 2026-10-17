use std::borrow::Cow;

use mcrs_minecraft_block::keys::Fluid;
use mcrs_minecraft_core::{BlockPos, ColumnPos};
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::IntoDeserializer;
use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A pending fluid tick of a saved chunk. A source and a flowing state of one
/// fluid share the fluid's id, so a flowing id names no fluid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedFluidTick {
    pub fluid: Fluid,
    pub pos: BlockPos,
    pub delay: i32,
    pub priority: i32,
}

/// A saved `fluid_ticks` element that did not load. The rest of the list
/// loads without it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DroppedFluidTick {
    UnknownFluid(String),
    Malformed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkDecodeReport {
    pub dropped_fluid_ticks: Vec<DroppedFluidTick>,
}

impl ChunkDecodeReport {
    pub fn is_clean(&self) -> bool {
        self.dropped_fluid_ticks.is_empty()
    }
}

#[derive(Serialize, Deserialize)]
struct RawTick<I> {
    i: I,
    x: i32,
    y: i32,
    z: i32,
    t: i32,
    p: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FluidTicks<'a> {
    pub(crate) ticks: Cow<'a, [SavedFluidTick]>,
    pub(crate) dropped: Vec<DroppedFluidTick>,
}

impl FluidTicks<'_> {
    /// A tick outside the column belongs to no cell of it, so it is neither
    /// kept nor reported.
    pub(crate) fn within(self, column: ColumnPos) -> (Vec<SavedFluidTick>, Vec<DroppedFluidTick>) {
        let mut ticks = self.ticks.into_owned();
        ticks.retain(|tick| ColumnPos::from(tick.pos) == column);
        (ticks, self.dropped)
    }
}

fn decode(tag: NbtTag) -> Result<SavedFluidTick, DroppedFluidTick> {
    let raw: RawTick<String> = mcrs_minecraft_nbt::from_tag(tag)
        .map_err(|err| DroppedFluidTick::Malformed(err.to_string()))?;
    let id: StrDeserializer<'_, ValueError> = raw.i.as_str().into_deserializer();
    let fluid =
        Fluid::deserialize(id).map_err(|_| DroppedFluidTick::UnknownFluid(raw.i.clone()))?;
    Ok(SavedFluidTick {
        fluid,
        pos: BlockPos::new(raw.x, raw.y, raw.z),
        delay: raw.t,
        priority: raw.p,
    })
}

impl<'de> Deserialize<'de> for FluidTicks<'_> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut ticks = Vec::new();
        let mut dropped = Vec::new();
        // The game reads the list with a fallback to none, so a tag of another
        // type loses the ticks, not the chunk.
        let tags = match <NbtTag as Deserialize>::deserialize(d)? {
            NbtTag::List(tags) => tags,
            _ => Vec::new(),
        };
        for tag in tags {
            match decode(tag) {
                Ok(tick) => ticks.push(tick),
                Err(reason) => dropped.push(reason),
            }
        }
        Ok(FluidTicks {
            ticks: Cow::Owned(ticks),
            dropped,
        })
    }
}

impl Serialize for FluidTicks<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.ticks.iter().map(|tick| RawTick {
            i: tick.fluid,
            x: tick.pos.x,
            y: tick.pos.y,
            z: tick.pos.z,
            t: tick.delay,
            p: tick.priority,
        }))
    }
}
