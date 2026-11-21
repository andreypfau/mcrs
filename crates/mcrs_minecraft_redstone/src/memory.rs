use mcrs_minecraft_core::Direction;
use mcrs_minecraft_simulation::GameTick;
use mcrs_voxel::{BlockPos, BlockStateId};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::wire_network::WireScratch;

pub(crate) const TOGGLE_WINDOW: i64 = 60;
pub(crate) const BURNOUT_TOGGLES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Toggle {
    pos: BlockPos,
    at: GameTick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carried {
    Block,
    Head,
    Base,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovingPayload {
    pub state: BlockStateId,
    pub facing: Direction,
    pub carried: Carried,
    pub started: GameTick,
}

impl MovingPayload {
    #[must_use]
    pub fn is_extending(&self) -> bool {
        self.carried != Carried::Base
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Presence {
    pub entities: u16,
    pub living: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalogSource {
    Container(u8),
    Frame { facing: Direction, signal: u8 },
}

#[derive(Clone, Debug, Default)]
pub struct RedstoneMemory {
    toggles: Vec<Toggle>,
    pub comparators: FxHashMap<BlockPos, u8>,
    pub comparator_dirty: Vec<BlockPos>,
    pub moving: FxHashMap<BlockPos, MovingPayload>,
    pub plates: FxHashMap<BlockPos, Presence>,
    pub plate_positions: FxHashSet<BlockPos>,
    pub projectiles: FxHashMap<BlockPos, u16>,
    pub analog: FxHashMap<BlockPos, AnalogSource>,
    pub wire_scratch: WireScratch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleLog {
    Record,
    OnlyCount,
}

impl RedstoneMemory {
    pub fn burns_out(&mut self, pos: BlockPos, now: GameTick, log: ToggleLog) -> bool {
        self.toggles
            .retain(|toggle| now.0.saturating_sub(toggle.at.0) <= TOGGLE_WINDOW);
        if log == ToggleLog::Record {
            self.toggles.push(Toggle { pos, at: now });
        }
        self.toggles
            .iter()
            .filter(|toggle| toggle.pos == pos)
            .count()
            >= BURNOUT_TOGGLES
    }

    #[must_use]
    pub fn comparator_output(&self, pos: BlockPos) -> u8 {
        self.comparators.get(&pos).copied().unwrap_or(0)
    }

    #[must_use]
    pub fn presence(&self, pos: BlockPos) -> Presence {
        self.plates.get(&pos).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn container_output(&self, pos: BlockPos) -> Option<u8> {
        match self.analog.get(&pos) {
            Some(AnalogSource::Container(signal)) => Some(*signal),
            _ => None,
        }
    }

    #[must_use]
    pub fn frame_output(&self, pos: BlockPos, facing: Direction) -> Option<u8> {
        match self.analog.get(&pos) {
            Some(AnalogSource::Frame {
                facing: hung,
                signal,
            }) if *hung == facing => Some(*signal),
            _ => None,
        }
    }

    #[must_use]
    pub fn projectiles_in(&self, pos: BlockPos) -> u16 {
        self.projectiles.get(&pos).copied().unwrap_or(0)
    }
}

pub trait HasRedstoneMemory {
    fn redstone(&mut self) -> &mut RedstoneMemory;

    fn redstone_ref(&self) -> &RedstoneMemory;
}

impl HasRedstoneMemory for RedstoneMemory {
    fn redstone(&mut self) -> &mut RedstoneMemory {
        self
    }

    fn redstone_ref(&self) -> &RedstoneMemory {
        self
    }
}
