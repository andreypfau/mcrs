use mcrs_minecraft_simulation::{
    PRECIPITATION_TILE, RandomHit, RandomPick, RandomTickTable, SectionStreams, TickingSectionMut,
    draw_random_picks,
};
use mcrs_minecraft_volume::SectionBlocksMut;
use mcrs_voxel::SectionPos;

use crate::world::storage::indexed_section::IndexedSection;

const COLUMNS: usize = (PRECIPITATION_TILE * PRECIPITATION_TILE) as usize;

#[derive(Debug, Default)]
pub(crate) struct BlockTile {
    sections: Vec<IndexedSection>,
    listed: Vec<u16>,
    columns: [u16; COLUMNS + 1],
    random_ticked_columns: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Listing {
    Fresh,
    Stale,
}

fn is_listed(sections: &[IndexedSection], slot: u16) -> bool {
    sections
        .get(usize::from(slot))
        .is_some_and(IndexedSection::is_listed)
}

fn column_in_tile(pos: SectionPos) -> usize {
    (pos.x.rem_euclid(PRECIPITATION_TILE) * PRECIPITATION_TILE
        + pos.z.rem_euclid(PRECIPITATION_TILE)) as usize
}

impl BlockTile {
    pub(crate) fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub(crate) fn is_random_ticked(&self) -> bool {
        self.random_ticked_columns != 0
    }

    fn slot(&self, pos: SectionPos) -> Result<usize, usize> {
        let column = column_in_tile(pos);
        let start = self
            .columns
            .get(column)
            .map_or(0, |&start| usize::from(start));
        let end = self
            .columns
            .get(column + 1)
            .map_or(start, |&end| usize::from(end));
        let column = self.sections.get(start..end).unwrap_or_default();
        let guess = column
            .first()
            .and_then(|lowest| usize::try_from(pos.y.wrapping_sub(lowest.pos.y)).ok());
        if let Some(guess) = guess
            && column.get(guess).is_some_and(|section| section.pos == pos)
        {
            return Ok(start + guess);
        }
        column
            .binary_search_by_key(&pos.y, |section| section.pos.y)
            .map(|slot| start + slot)
            .map_err(|slot| start + slot)
    }

    fn mark_random_ticked_columns(&mut self) {
        self.random_ticked_columns = self
            .sections
            .iter()
            .filter(|section| section.random_ticked)
            .fold(0, |mask, section| mask | 1 << column_in_tile(section.pos));
    }

    fn relist(&mut self) {
        self.mark_random_ticked_columns();
        let mut slot = 0;
        for (column, start) in self.columns.iter_mut().enumerate() {
            *start = slot;
            while self
                .sections
                .get(usize::from(slot))
                .is_some_and(|section| column_in_tile(section.pos) == column)
            {
                slot += 1;
            }
        }
        self.listed.clear();
        self.listed.extend(
            (0u16..)
                .zip(&self.sections)
                .filter(|(_, section)| section.is_listed())
                .map(|(slot, _)| slot),
        );
    }

    pub(crate) fn prune(&mut self) {
        let Self {
            sections, listed, ..
        } = self;
        listed.retain(|&slot| is_listed(sections, slot));
    }

    fn list(&mut self, slot: usize) {
        let Ok(slot) = u16::try_from(slot) else {
            return;
        };
        if let Err(at) = self.listed.binary_search(&slot) {
            self.listed.insert(at, slot);
        }
    }

    pub(crate) fn upsert(&mut self, section: IndexedSection) {
        match self.slot(section.pos) {
            Ok(slot) => {
                if let Some(held) = self.sections.get_mut(slot) {
                    *held = section;
                }
            }
            Err(slot) => self.sections.insert(slot, section),
        }
        self.relist();
    }

    pub(crate) fn remove(&mut self, pos: SectionPos) {
        if let Ok(slot) = self.slot(pos) {
            self.sections.remove(slot);
            self.relist();
        }
    }

    pub(crate) fn set_random_ticked(&mut self, pos: SectionPos, random_ticked: bool) {
        let Ok(slot) = self.slot(pos) else {
            return;
        };
        let Some(section) = self.sections.get_mut(slot) else {
            return;
        };
        section.random_ticked = random_ticked;
        if section.is_listed() {
            self.list(slot);
        }
        self.mark_random_ticked_columns();
    }

    #[cfg(test)]
    pub(crate) fn ticking(&self, pos: SectionPos) -> &[u16] {
        self.slot(pos)
            .ok()
            .and_then(|slot| self.sections.get(slot))
            .map_or(&[], |section| section.ticking.as_slice())
    }

    pub(crate) fn section_mut<'a, S: SectionBlocksMut>(
        &'a mut self,
        pos: SectionPos,
        blocks: S,
        table: &'a RandomTickTable,
    ) -> TickingSectionMut<'a, S> {
        let Ok(slot) = self.slot(pos) else {
            return TickingSectionMut::untracked(blocks);
        };
        if self
            .sections
            .get(slot)
            .is_some_and(|section| section.random_ticked)
        {
            self.list(slot);
        }
        match self.sections.get_mut(slot) {
            Some(section) => TickingSectionMut::new(blocks, &mut section.ticking, table),
            None => TickingSectionMut::untracked(blocks),
        }
    }

    pub(crate) fn owned_columns(&self, mut owns: impl FnMut(i32) -> bool) -> u16 {
        (0..PRECIPITATION_TILE * PRECIPITATION_TILE)
            .filter(|&column| self.random_ticked_columns & 1 << column != 0)
            .filter(|&column| owns(column))
            .fold(0u16, |mask, column| mask | 1 << column)
    }

    pub(crate) fn drawn_slots(&self, drawn: &mut Vec<u16>) -> Listing {
        drawn.clone_from(&self.listed);
        drawn.retain(|&slot| is_listed(&self.sections, slot));
        if drawn.len() < self.listed.len() {
            Listing::Stale
        } else {
            Listing::Fresh
        }
    }

    pub(crate) fn draw_picks(
        &self,
        slot: u16,
        owned: u16,
        speed: u32,
        streams: SectionStreams,
        picks: &mut Vec<RandomPick>,
    ) {
        if let Some(section) = self.sections.get(usize::from(slot))
            && owned & 1 << column_in_tile(section.pos) != 0
        {
            let ticking = section.ticking.as_slice().len();
            draw_random_picks(section.pos, ticking, speed, streams, picks);
        }
    }

    pub(crate) fn extend_hits(&self, slot: u16, pick: RandomPick, hits: &mut Vec<RandomHit>) {
        if let Some(section) = self.sections.get(usize::from(slot)) {
            hits.extend(RandomHit::of(section.pos, section.ticking.as_slice(), pick));
        }
    }
}
