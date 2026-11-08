use crate::random_tick::RandomTickTable;
use mcrs_minecraft_volume::{Palette, PaletteBuffer, SectionBlocks, SectionBlocksMut};
use mcrs_voxel::{BlockStateId, LocalPos};

/// The local indices of the positions of one section whose state takes random ticks, in
/// ascending order: the list depends only on the states, so undoing a write restores it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickingPositions {
    positions: Vec<u16>,
}

impl TickingPositions {
    /// Lists the positions of `section` whose state takes random ticks under `table`.
    pub fn rebuild<S: SectionBlocks + ?Sized>(
        &mut self,
        section: &S,
        table: &RandomTickTable,
        palette: &mut PaletteBuffer,
    ) {
        self.positions.clear();
        if !section.palette(palette).any(|state| table.ticks(state)) {
            return;
        }
        section.for_each(|at, state| {
            if table.ticks(state) {
                self.positions.push(local_index(at));
            }
        });
    }

    /// Follows a write at `at` that replaced `old` with `new`.
    pub fn record(
        &mut self,
        at: LocalPos,
        old: BlockStateId,
        new: BlockStateId,
        table: &RandomTickTable,
    ) {
        let found = self.positions.binary_search(&local_index(at));
        match (table.ticks(old), table.ticks(new), found) {
            (false, true, Err(slot)) => self.positions.insert(slot, local_index(at)),
            (true, false, Ok(slot)) => {
                self.positions.remove(slot);
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u16] {
        &self.positions
    }
}

pub(crate) fn local_index(at: LocalPos) -> u16 {
    // A section holds 4096 positions, so every local index fits.
    at.index() as u16
}

/// A section handle whose writes keep the [`TickingPositions`] of the section.
#[derive(Debug)]
pub struct TickingSectionMut<'a, S> {
    section: S,
    tracker: Option<Tracker<'a>>,
}

#[derive(Debug)]
struct Tracker<'a> {
    ticking: &'a mut TickingPositions,
    table: &'a RandomTickTable,
}

impl<'a, S: SectionBlocksMut> TickingSectionMut<'a, S> {
    /// Keeps `ticking` with each write to `section`.
    pub fn new(section: S, ticking: &'a mut TickingPositions, table: &'a RandomTickTable) -> Self {
        Self {
            section,
            tracker: Some(Tracker { ticking, table }),
        }
    }

    /// Writes to `section` without a list to keep: a host that lists no random ticks.
    pub fn untracked(section: S) -> Self {
        Self {
            section,
            tracker: None,
        }
    }

    fn rebuild(&mut self) {
        if let Some(tracker) = self.tracker.as_mut() {
            tracker
                .ticking
                .rebuild(&self.section, tracker.table, &mut PaletteBuffer::default());
        }
    }
}

impl<S: SectionBlocksMut> SectionBlocks for TickingSectionMut<'_, S> {
    #[inline]
    fn get(&self, at: LocalPos) -> BlockStateId {
        self.section.get(at)
    }

    fn palette<'s>(&'s self, buffer: &'s mut PaletteBuffer) -> Palette<'s> {
        self.section.palette(buffer)
    }
}

impl<S: SectionBlocksMut> SectionBlocksMut for TickingSectionMut<'_, S> {
    fn set(&mut self, at: LocalPos, state: BlockStateId) -> BlockStateId {
        let old = self.section.set(at, state);
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.ticking.record(at, old, state, tracker.table);
        }
        old
    }

    fn fill(&mut self, state: BlockStateId) {
        self.section.fill(state);
        self.rebuild();
    }

    fn fill_box(&mut self, min: LocalPos, max: LocalPos, state: BlockStateId) {
        self.section.fill_box(min, max, state);
        self.rebuild();
    }
}
