use std::borrow::Cow;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_voxel::{BlockPos, ColumnPos};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A pending tick of a saved chunk. A source and a flowing state of one
/// fluid share the fluid's id, so a flowing id names no fluid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedTick<K> {
    pub kind: K,
    pub pos: BlockPos,
    pub delay: i32,
    pub priority: i32,
}

pub type SavedBlockTick = SavedTick<Block>;
pub type SavedFluidTick = SavedTick<Fluid>;

const MIN_PRIORITY: i32 = -3;
const MAX_PRIORITY: i32 = 3;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTick<I> {
    i: I,
    x: i32,
    y: i32,
    z: i32,
    t: i32,
    p: i32,
}

/// The `block_ticks` or `fluid_ticks` list of a record. An entry that does not load fails the
/// record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TickList<'a, K: Clone> {
    pub(crate) ticks: Cow<'a, [SavedTick<K>]>,
}

pub(crate) type BlockTicks<'a> = TickList<'a, Block>;
pub(crate) type FluidTicks<'a> = TickList<'a, Fluid>;

impl<K: Clone> Default for TickList<'_, K> {
    fn default() -> Self {
        TickList {
            ticks: Cow::Owned(Vec::new()),
        }
    }
}

impl<K: Clone> TickList<'_, K> {
    /// A tick outside the column belongs to no cell of it.
    pub(crate) fn within(self, column: ColumnPos) -> Vec<SavedTick<K>> {
        let mut ticks = self.ticks.into_owned();
        ticks.retain(|tick| ColumnPos::from(tick.pos) == column);
        ticks
    }
}

impl<'de, K: Clone + Deserialize<'de>> Deserialize<'de> for TickList<'_, K> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(TickList {
            ticks: Cow::Owned(Vec::deserialize(d)?),
        })
    }
}

impl<K: Clone + Serialize> Serialize for TickList<'_, K> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.ticks.iter())
    }
}

impl<K: Serialize> Serialize for SavedTick<K> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        RawTick {
            i: &self.kind,
            x: self.pos.x,
            y: self.pos.y,
            z: self.pos.z,
            t: self.delay,
            p: self.priority,
        }
        .serialize(s)
    }
}

impl<'de, K: Deserialize<'de>> Deserialize<'de> for SavedTick<K> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = RawTick::<K>::deserialize(d)?;
        Ok(SavedTick {
            kind: raw.i,
            pos: BlockPos::new(raw.x, raw.y, raw.z),
            delay: raw.t,
            priority: raw.p.clamp(MIN_PRIORITY, MAX_PRIORITY),
        })
    }
}
