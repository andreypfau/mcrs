use mcrs_minecraft_core::Direction;
use mcrs_minecraft_simulation::{GameTick, MemoryPartition, memory_cell};
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};
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
pub enum MovePhase {
    Extending,
    Retracting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovingPayload {
    pub state: BlockStateId,
    pub facing: Direction,
    pub carried: Carried,
    pub phase: MovePhase,
    pub started: GameTick,
}

impl MovingPayload {
    #[must_use]
    pub fn is_extending(&self) -> bool {
        self.phase == MovePhase::Extending
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
struct RedstoneCell {
    toggles: FxHashMap<BlockPos, ToggleRing>,
    comparators: FxHashMap<BlockPos, u8>,
    comparator_dirty: Vec<BlockPos>,
    moving: FxHashMap<BlockPos, MovingPayload>,
    sensors: FxHashMap<BlockPos, Sensor>,
}

impl RedstoneCell {
    fn is_empty(&self) -> bool {
        self.toggles.is_empty()
            && self.comparators.is_empty()
            && self.comparator_dirty.is_empty()
            && self.moving.is_empty()
            && self.sensors.is_empty()
    }

    fn absorb(&mut self, other: Self) {
        self.toggles.extend(other.toggles);
        self.comparators.extend(other.comparators);
        self.moving.extend(other.moving);
        self.sensors.extend(other.sensors);
        for pos in other.comparator_dirty {
            if !self.comparator_dirty.contains(&pos) {
                self.comparator_dirty.push(pos);
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RedstoneMemory {
    cells: FxHashMap<u64, RedstoneCell>,
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
    fn cell(&self, pos: BlockPos) -> Option<&RedstoneCell> {
        self.cells.get(&memory_cell(pos))
    }

    fn cell_mut(&mut self, pos: BlockPos) -> &mut RedstoneCell {
        self.cells.entry(memory_cell(pos)).or_default()
    }

    fn prune(&mut self, pos: BlockPos) {
        let key = memory_cell(pos);
        if self.cells.get(&key).is_some_and(RedstoneCell::is_empty) {
            self.cells.remove(&key);
        }
    }

    pub fn burns_out(&mut self, pos: BlockPos, now: GameTick, log: ToggleLog) -> bool {
        let cell = self.cell_mut(pos);
        if cell.toggles.len() >= TOGGLE_SWEEP {
            cell.toggles.retain(|_, ring| !ring.is_stale(now.0));
        }
        let ring = cell.toggles.entry(pos).or_default();
        if log == ToggleLog::Record {
            ring.record(now.0);
        }
        let burnt = ring.recent(now.0) >= BURNOUT_TOGGLES;
        if ring.is_stale(now.0) {
            cell.toggles.remove(&pos);
            self.prune(pos);
        }
        burnt
    }

    #[must_use]
    pub fn comparator_output(&self, pos: BlockPos) -> u8 {
        self.cell(pos)
            .and_then(|cell| cell.comparators.get(&pos).copied())
            .unwrap_or(0)
    }

    pub fn set_comparator_output(&mut self, pos: BlockPos, output: u8, changed: bool) {
        let cell = self.cell_mut(pos);
        cell.comparators.insert(pos, output);
        if changed && !cell.comparator_dirty.contains(&pos) {
            cell.comparator_dirty.push(pos);
        }
    }

    pub fn restore_comparator_output(&mut self, pos: BlockPos, output: u8) {
        self.cell_mut(pos).comparators.entry(pos).or_insert(output);
    }

    pub fn forget_comparator(&mut self, pos: BlockPos) {
        self.cell_mut(pos).comparators.remove(&pos);
        self.prune(pos);
    }

    #[must_use]
    pub fn has_dirty_comparators(&self) -> bool {
        self.cells
            .values()
            .any(|cell| !cell.comparator_dirty.is_empty())
    }

    pub fn dirty_comparators(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.cells
            .values()
            .flat_map(|cell| cell.comparator_dirty.iter().copied())
    }

    pub fn take_dirty_comparators(&mut self) -> Vec<(BlockPos, u8)> {
        let mut taken = Vec::new();
        for cell in self.cells.values_mut() {
            for pos in cell.comparator_dirty.drain(..) {
                if let Some(signal) = cell.comparators.get(&pos) {
                    taken.push((pos, *signal));
                }
            }
        }
        taken.sort_unstable_by_key(|(pos, _)| (pos.x, pos.y, pos.z));
        taken
    }

    #[must_use]
    pub fn moving(&self, pos: BlockPos) -> Option<MovingPayload> {
        self.cell(pos)
            .and_then(|cell| cell.moving.get(&pos).copied())
    }

    pub fn start_moving(&mut self, pos: BlockPos, payload: MovingPayload) {
        self.cell_mut(pos).moving.insert(pos, payload);
    }

    pub fn stop_moving(&mut self, pos: BlockPos) -> Option<MovingPayload> {
        let payload = self.cell_mut(pos).moving.remove(&pos);
        self.prune(pos);
        payload
    }

    #[must_use]
    pub fn has_moving_blocks(&self) -> bool {
        self.cells.values().any(|cell| !cell.moving.is_empty())
    }

    pub fn moving_blocks(&self) -> impl Iterator<Item = (&BlockPos, &MovingPayload)> {
        self.cells.values().flat_map(|cell| cell.moving.iter())
    }

    #[must_use]
    pub fn sensor(&self, pos: BlockPos) -> Option<Sensor> {
        self.cell(pos)
            .and_then(|cell| cell.sensors.get(&pos).copied())
    }

    pub fn add_sensor(&mut self, pos: BlockPos, sensor: Sensor) {
        self.cell_mut(pos).sensors.insert(pos, sensor);
    }

    pub fn remove_sensor(&mut self, pos: BlockPos) {
        self.cell_mut(pos).sensors.remove(&pos);
        self.prune(pos);
    }

    pub fn remove_sensors_in(&mut self, section: SectionPos) {
        let origin = section.min_block();
        if let Some(cell) = self.cells.get_mut(&memory_cell(origin)) {
            cell.sensors
                .retain(|pos, _| SectionPos::from(*pos) != section);
        }
        self.prune(origin);
    }

    #[must_use]
    pub fn has_sensors(&self) -> bool {
        self.cells.values().any(|cell| !cell.sensors.is_empty())
    }

    pub fn sensors(&self) -> impl Iterator<Item = (BlockPos, Sensor)> + '_ {
        self.cells
            .values()
            .flat_map(|cell| cell.sensors.iter().map(|(pos, sensor)| (*pos, *sensor)))
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

impl MemoryPartition for RedstoneMemory {
    fn split(&mut self) -> Vec<(u64, Self)> {
        self.cells
            .drain()
            .filter(|(_, cell)| !cell.is_empty())
            .map(|(key, cell)| {
                let mut part = RedstoneMemory::default();
                part.cells.insert(key, cell);
                (key, part)
            })
            .collect()
    }

    fn merge(&mut self, part: Self) {
        for (key, cell) in part.cells {
            match self.cells.get_mut(&key) {
                Some(held) => held.absorb(cell),
                None => {
                    self.cells.insert(key, cell);
                }
            }
        }
    }

    fn copy_cell(&self, cell: u64) -> Self {
        let mut part = Self::default();
        if let Some(found) = self.cells.get(&cell) {
            part.cells.insert(cell, found.clone());
        }
        part
    }

    fn drop_cell(&mut self, cell: u64) {
        self.cells.remove(&cell);
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
