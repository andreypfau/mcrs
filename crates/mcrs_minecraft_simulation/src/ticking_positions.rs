use crate::random_tick::RandomTickTable;
use mcrs_minecraft_volume::{Palette, PaletteBuffer, SectionBlocks, SectionBlocksMut};
use mcrs_voxel::{BlockStateId, LocalPos};

/// The local indices of the positions of one section whose state takes random ticks, in no
/// fixed order.
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
        match (table.ticks(old), table.ticks(new)) {
            (false, true) => self.positions.push(local_index(at)),
            (true, false) => {
                let index = local_index(at);
                if let Some(slot) = self.positions.iter().position(|&listed| listed == index) {
                    self.positions.swap_remove(slot);
                }
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u16] {
        &self.positions
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

fn local_index(at: LocalPos) -> u16 {
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

    fn uniform(&self) -> Option<BlockStateId> {
        self.section.uniform()
    }

    fn palette<'s>(&'s self, buffer: &'s mut PaletteBuffer) -> Palette<'s> {
        self.section.palette(buffer)
    }

    fn for_each<F>(&self, f: F)
    where
        F: FnMut(LocalPos, BlockStateId),
    {
        self.section.for_each(f);
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
