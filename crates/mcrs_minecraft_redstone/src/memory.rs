use mcrs_minecraft_core::Direction;
use mcrs_minecraft_simulation::GameTick;
use mcrs_voxel::{BlockPos, BlockStateId};
use rustc_hash::FxHashMap;

pub(crate) const TOGGLE_WINDOW: i64 = 60;
pub(crate) const BURNOUT_TOGGLES: usize = 8;
const TOGGLE_SWEEP: usize = 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ToggleRing {
    times: [i64; BURNOUT_TOGGLES],
    len: u8,
    next: u8,
}

impl ToggleRing {
    fn record(&mut self, now: i64) {
        if let Some(slot) = self.times.get_mut(usize::from(self.next)) {
            *slot = now;
        }
        self.next = (self.next.wrapping_add(1)) % BURNOUT_TOGGLES as u8;
        self.len = self.len.saturating_add(1).min(BURNOUT_TOGGLES as u8);
    }

    fn recent(&self, now: i64) -> usize {
        self.times
            .iter()
            .take(usize::from(self.len))
            .filter(|at| now.saturating_sub(**at) <= TOGGLE_WINDOW)
            .count()
    }

    fn is_stale(&self, now: i64) -> bool {
        self.recent(now) == 0
    }
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
    Daylight { sun: u8, sky: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensor {
    Plate,
    Tripwire,
    Daylight,
}

#[derive(Clone, Debug, Default)]
pub struct RedstoneMemory {
    toggles: FxHashMap<BlockPos, ToggleRing>,
    pub comparators: FxHashMap<BlockPos, u8>,
    pub comparator_dirty: Vec<BlockPos>,
    pub moving: FxHashMap<BlockPos, MovingPayload>,
    pub sensors: FxHashMap<BlockPos, Sensor>,
}

#[derive(Clone, Debug, Default)]
pub struct RedstoneSnapshot {
    pub plates: FxHashMap<BlockPos, Presence>,
    pub projectiles: FxHashMap<BlockPos, u16>,
    pub analog: FxHashMap<BlockPos, AnalogSource>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleLog {
    Record,
    OnlyCount,
}

impl RedstoneMemory {
    pub fn burns_out(&mut self, pos: BlockPos, now: GameTick, log: ToggleLog) -> bool {
        if self.toggles.len() >= TOGGLE_SWEEP {
            self.toggles.retain(|_, ring| !ring.is_stale(now.0));
        }
        let ring = self.toggles.entry(pos).or_default();
        if log == ToggleLog::Record {
            ring.record(now.0);
        }
        let burnt = ring.recent(now.0) >= BURNOUT_TOGGLES;
        if ring.is_stale(now.0) {
            self.toggles.remove(&pos);
        }
        burnt
    }

    #[must_use]
    pub fn comparator_output(&self, pos: BlockPos) -> u8 {
        self.comparators.get(&pos).copied().unwrap_or(0)
    }
}

impl RedstoneSnapshot {
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

    #[must_use]
    pub fn daylight(&self, pos: BlockPos) -> Option<(u8, u8)> {
        match self.analog.get(&pos) {
            Some(AnalogSource::Daylight { sun, sky }) => Some((*sun, *sky)),
            _ => None,
        }
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

pub trait HasRedstoneSnapshot {
    fn redstone(&self) -> &RedstoneSnapshot;

    fn redstone_mut(&mut self) -> &mut RedstoneSnapshot;
}

impl HasRedstoneSnapshot for RedstoneSnapshot {
    fn redstone(&self) -> &RedstoneSnapshot {
        self
    }

    fn redstone_mut(&mut self) -> &mut RedstoneSnapshot {
        self
    }
}
