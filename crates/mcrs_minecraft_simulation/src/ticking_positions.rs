use crate::random_tick::RandomTickTable;
use mcrs_minecraft_volume::{Palette, PaletteBuffer, SectionBlocks, SectionBlocksMut};
use mcrs_voxel::{BlockStateId, LocalPos};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickingPositions {
    positions: Vec<u16>,
}

impl TickingPositions {
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
    at.index() as u16
}

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
    pub fn new(section: S, ticking: &'a mut TickingPositions, table: &'a RandomTickTable) -> Self {
        Self {
            section,
            tracker: Some(Tracker { ticking, table }),
        }
    }

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
