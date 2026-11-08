//! The positions that tick randomly in each loaded section of a dimension, by block tile.

use bevy_app::{App, Plugin};
use bevy_ecs::lifecycle::{Insert, Remove};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Query, ResMut};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_simulation::{
    PRECIPITATION_TILE, RandomHit, RandomPick, RandomTickTable, SectionStreams, TickingPositions,
    TickingSectionMut, block_tile, draw_random_picks,
};
use mcrs_minecraft_volume::{PaletteBuffer, SectionBlocks, SectionBlocksMut};
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::world::lifecycle::simulation::EntityTicking;

/// Keeps the [`RandomTickIndex`] of a dimension world with the sections that start or stop
/// random ticks. Each change of blocks goes through the
/// [`SectionIndex`](crate::world::storage::section::SectionIndex), which keeps the ticking
/// positions.
pub struct RandomTickIndexPlugin;

impl Plugin for RandomTickIndexPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(start_random_ticks)
            .add_observer(stop_random_ticks);
    }
}

/// The positions whose state ticks randomly in each loaded section, and for each block tile the
/// random-ticked sections that hold such a position.
#[derive(Resource, Debug)]
pub struct RandomTickIndex {
    table: RandomTickTable,
    tiles: FxHashMap<ColumnPos, BlockTile>,
    palette: PaletteBuffer,
    order: Vec<ColumnPos>,
    /// The picks of one tile that hit, with the slot of their section.
    picks: Vec<RandomPick>,
    picked: Vec<u16>,
}

#[derive(Debug, Default)]
struct BlockTile {
    /// Sorted by x, z, then y.
    sections: Vec<IndexedSection>,
    /// Slots of `sections`, sorted: each random-ticked section that holds a ticking position, and
    /// possibly some that no longer do.
    listed: Vec<u16>,
    /// The first slot of each column of the tile in `sections`, then the end of the last.
    columns: [u16; COLUMNS + 1],
    /// One bit for each column that holds a random-ticked section, at its index in the tile.
    random_ticked_columns: u16,
}

const COLUMNS: usize = (PRECIPITATION_TILE * PRECIPITATION_TILE) as usize;

#[derive(Debug)]
struct IndexedSection {
    pos: SectionPos,
    random_ticked: bool,
    ticking: TickingPositions,
}

impl IndexedSection {
    fn is_listed(&self) -> bool {
        self.random_ticked && !self.ticking.as_slice().is_empty()
    }
}

fn tile_of(pos: SectionPos) -> ColumnPos {
    block_tile(ColumnPos::from(pos))
}

/// The column of `pos` in its tile: ascending in the order of `BlockTile::sections`.
fn column_in_tile(pos: SectionPos) -> usize {
    (pos.x.rem_euclid(PRECIPITATION_TILE) * PRECIPITATION_TILE
        + pos.z.rem_euclid(PRECIPITATION_TILE)) as usize
}

impl BlockTile {
    /// The sections of a column lie in ascending y, mostly with no gap, so the slot of a section
    /// is first guessed from the lowest one: a lookup reads one section, not a binary search.
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

    fn list(&mut self, slot: usize) {
        let Ok(slot) = u16::try_from(slot) else {
            return;
        };
        if let Err(at) = self.listed.binary_search(&slot) {
            self.listed.insert(at, slot);
        }
    }
}

impl RandomTickIndex {
    /// An index of no section, under the random ticks of `table`.
    #[must_use]
    pub fn new(table: RandomTickTable) -> Self {
        Self {
            table,
            tiles: FxHashMap::default(),
            palette: PaletteBuffer::default(),
            order: Vec::new(),
            picks: Vec::new(),
            picked: Vec::new(),
        }
    }

    /// Lists the ticking positions of `blocks`, the section at `pos`, in place of what the index
    /// held for it.
    pub fn insert<S: SectionBlocks>(&mut self, pos: SectionPos, blocks: &S, random_ticked: bool) {
        let mut ticking = TickingPositions::default();
        ticking.rebuild(blocks, &self.table, &mut self.palette);
        let tile = self.tiles.entry(tile_of(pos)).or_default();
        let section = IndexedSection {
            pos,
            random_ticked,
            ticking,
        };
        match tile.slot(pos) {
            Ok(slot) => {
                if let Some(held) = tile.sections.get_mut(slot) {
                    *held = section;
                }
            }
            Err(slot) => tile.sections.insert(slot, section),
        }
        tile.relist();
    }

    pub(crate) fn remove(&mut self, pos: SectionPos) {
        let tile_pos = tile_of(pos);
        let Some(tile) = self.tiles.get_mut(&tile_pos) else {
            return;
        };
        if let Ok(slot) = tile.slot(pos) {
            tile.sections.remove(slot);
            tile.relist();
        }
        if tile.sections.is_empty() {
            self.tiles.remove(&tile_pos);
        }
    }

    fn set_random_ticked(&mut self, pos: SectionPos, random_ticked: bool) {
        let Some(tile) = self.tiles.get_mut(&tile_of(pos)) else {
            return;
        };
        let Ok(slot) = tile.slot(pos) else {
            return;
        };
        let Some(section) = tile.sections.get_mut(slot) else {
            return;
        };
        section.random_ticked = random_ticked;
        if section.is_listed() {
            tile.list(slot);
        }
        tile.mark_random_ticked_columns();
    }

    #[cfg(test)]
    fn ticking(&self, pos: SectionPos) -> &[u16] {
        self.tiles
            .get(&tile_of(pos))
            .and_then(|tile| tile.sections.get(tile.slot(pos).ok()?))
            .map_or(&[], |section| section.ticking.as_slice())
    }

    /// A handle that writes `blocks`, the section at `pos`, and keeps its ticking positions.
    pub(crate) fn section_mut<'a, S: SectionBlocksMut>(
        &'a mut self,
        pos: SectionPos,
        blocks: S,
    ) -> TickingSectionMut<'a, S> {
        let Some(tile) = self.tiles.get_mut(&tile_of(pos)) else {
            return TickingSectionMut::untracked(blocks);
        };
        let Ok(slot) = tile.slot(pos) else {
            return TickingSectionMut::untracked(blocks);
        };
        // The write can make the section list a position; `listed` may hold more than it needs.
        if tile
            .sections
            .get(slot)
            .is_some_and(|section| section.random_ticked)
        {
            tile.list(slot);
        }
        match tile.sections.get_mut(slot) {
            Some(section) => TickingSectionMut::new(blocks, &mut section.ticking, &self.table),
            None => TickingSectionMut::untracked(blocks),
        }
    }

    /// Replaces `out` with what takes random ticks in the columns that `owns` accepts, block tile
    /// by block tile in the order of the tiles: the hits of the `speed` picks of each
    /// random-ticked section from `streams`, in the order of the sections and the picks, and the
    /// columns that hold a random-ticked section, sorted by x, then z.
    pub fn draw(
        &mut self,
        owns: impl Fn(ColumnPos) -> bool,
        speed: u32,
        streams: SectionStreams,
        out: &mut RandomListing,
    ) {
        out.hits.clear();
        out.columns.clear();
        let Self {
            tiles,
            order,
            picks,
            picked,
            ..
        } = self;
        order.clear();
        order.extend(
            tiles
                .iter()
                .filter(|(_, tile)| tile.random_ticked_columns != 0)
                .map(|(pos, _)| *pos),
        );
        order.sort_unstable();
        for tile_pos in &*order {
            let Some(tile) = tiles.get_mut(tile_pos) else {
                continue;
            };
            let BlockTile {
                sections,
                listed,
                random_ticked_columns,
                ..
            } = tile;
            let column_pos = |column: i32| {
                ColumnPos::new(
                    tile_pos.x * PRECIPITATION_TILE + column / PRECIPITATION_TILE,
                    tile_pos.z * PRECIPITATION_TILE + column % PRECIPITATION_TILE,
                )
            };
            let columns = 0..PRECIPITATION_TILE * PRECIPITATION_TILE;
            let owned = columns
                .clone()
                .filter(|&column| *random_ticked_columns & 1 << column != 0)
                .filter(|&column| owns(column_pos(column)))
                .fold(0u16, |mask, column| mask | 1 << column);
            listed.retain(|&slot| {
                sections
                    .get(usize::from(slot))
                    .is_some_and(IndexedSection::is_listed)
            });
            for &slot in listed.iter() {
                let Some(section) = sections.get(usize::from(slot)) else {
                    continue;
                };
                if owned & 1 << column_in_tile(section.pos) != 0 {
                    let ticking = section.ticking.as_slice().len();
                    draw_random_picks(section.pos, ticking, speed, streams, picks);
                    picked.resize(picks.len(), slot);
                }
            }
            // The lists of the hits are read after all draws of the tile, so their cache misses
            // overlap.
            for (slot, pick) in picked.drain(..).zip(picks.drain(..)) {
                if let Some(section) = sections.get(usize::from(slot)) {
                    let ticking = section.ticking.as_slice();
                    out.hits.extend(RandomHit::of(section.pos, ticking, pick));
                }
            }
            out.columns.extend(
                columns
                    .filter(|&column| owned & 1 << column != 0)
                    .map(column_pos),
            );
        }
    }
}

/// What takes random ticks in one step, as [`RandomTickIndex::draw`] gives it.
#[derive(Clone, Debug, Default)]
pub struct RandomListing {
    pub hits: Vec<RandomHit>,
    pub columns: Vec<ColumnPos>,
}

fn start_random_ticks(
    insert: On<Insert, EntityTicking>,
    sections: Query<&SectionPos>,
    index: Option<ResMut<RandomTickIndex>>,
) {
    if let (Some(mut index), Ok(pos)) = (index, sections.get(insert.event().entity)) {
        index.set_random_ticked(*pos, true);
    }
}

fn stop_random_ticks(
    remove: On<Remove, EntityTicking>,
    sections: Query<&SectionPos>,
    index: Option<ResMut<RandomTickIndex>>,
) {
    if let (Some(mut index), Ok(pos)) = (index, sections.get(remove.event().entity)) {
        index.set_random_ticked(*pos, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{ChunkBlocks, SectionStates, air_section};
    use mcrs_minecraft_simulation::{GameTick, RandomTicks, StreamSeed};
    use mcrs_minecraft_volume::PalettedSection;
    use mcrs_voxel::BlockStateId;

    /// One pick in each listed section: in a section of grass it always hits.
    fn draw(index: &mut RandomTickIndex, owns: impl Fn(ColumnPos) -> bool) -> RandomListing {
        let mut out = RandomListing::default();
        let streams = StreamSeed::new(0, "minecraft:overworld").sections(GameTick(0));
        index.draw(owns, 1, streams, &mut out);
        out
    }

    fn listing(app: &mut App) -> RandomListing {
        draw(
            &mut app.world_mut().resource_mut::<RandomTickIndex>(),
            |_| true,
        )
    }

    fn picked(listing: &RandomListing) -> Vec<SectionPos> {
        listing
            .hits
            .iter()
            .map(|hit| SectionPos::from(hit.at))
            .collect()
    }

    fn listed(app: &mut App) -> Vec<SectionPos> {
        picked(&listing(app))
    }

    #[test]
    fn each_section_of_a_tile_answers_with_its_own_ticking_positions() {
        let grass = BlockStateId::checked(1, 2).expect("a state past air");
        let mut index = RandomTickIndex::new(RandomTickTable::new([grass], |_| RandomTicks::BLOCK));
        let grass_section = ChunkBlocks::new(SectionStates::homogeneous(grass));
        let air = air_section();
        // Two columns of one tile, inserted out of order, with gaps; grass and air alternate so
        // that a lookup one slot off answers wrong.
        let sections = [
            (5, 2, 9),
            (4, 1, 8),
            (5, 0, 9),
            (4, 3, 8),
            (5, -3, 9),
            (4, -1, 8),
        ];
        let ticks = |y: i32| y.rem_euclid(2) == 0;
        for (x, y, z) in sections {
            let blocks = if ticks(y) { &grass_section } else { &air };
            index.insert(
                SectionPos::new(x, y, z),
                &PalettedSection(&blocks.0.0),
                true,
            );
        }
        let check = |index: &RandomTickIndex, removed: Option<(i32, i32, i32)>| {
            for (x, y, z) in sections {
                let expected = if ticks(y) && Some((x, y, z)) != removed {
                    4096
                } else {
                    0
                };
                assert_eq!(
                    index.ticking(SectionPos::new(x, y, z)).len(),
                    expected,
                    "{x} {y} {z}"
                );
            }
            assert!(index.ticking(SectionPos::new(5, 1, 9)).is_empty(), "a gap");
            assert!(
                index.ticking(SectionPos::new(6, 0, 9)).is_empty(),
                "no column"
            );
        };
        check(&index, None);

        index.remove(SectionPos::new(5, 0, 9));
        check(&index, Some((5, 0, 9)));
    }

    /// SIM-RND-004: a column takes precipitation while it holds a random-ticked section, whatever
    /// its blocks are; a column of another cell takes neither precipitation nor random picks.
    #[test]
    fn a_listing_holds_the_owned_columns_of_random_ticked_sections() {
        let grass = BlockStateId::checked(1, 2).expect("a state past air");
        let mut index = RandomTickIndex::new(RandomTickTable::new([grass], |_| RandomTicks::BLOCK));
        let grass_section = ChunkBlocks::new(SectionStates::homogeneous(grass));
        let air = air_section();
        let sections = [
            (SectionPos::new(1, 0, 2), &grass_section, true),
            (SectionPos::new(0, 5, 3), &air, true),
            (SectionPos::new(0, 4, 0), &grass_section, false),
            (SectionPos::new(-3, 0, 0), &grass_section, true),
            (SectionPos::new(9, 0, 0), &grass_section, true),
        ];
        for (pos, blocks, random_ticked) in sections {
            index.insert(pos, &PalettedSection(&blocks.0.0), random_ticked);
        }
        let listing = draw(&mut index, |column| column.x < 8);

        assert_eq!(
            picked(&listing),
            [SectionPos::new(-3, 0, 0), SectionPos::new(1, 0, 2)]
        );
        assert_eq!(
            listing.columns,
            [
                ColumnPos::new(-3, 0),
                ColumnPos::new(0, 3),
                ColumnPos::new(1, 2)
            ],
            "by tile, then by x and z; the air section rains, the section out of the area does not"
        );
    }

    #[test]
    fn a_section_is_listed_while_it_is_loaded_random_ticked_and_holds_a_ticking_state() {
        let grass = BlockStateId::checked(1, 2).expect("a state past air");
        let table = RandomTickTable::new([grass], |_| RandomTicks::BLOCK);
        let mut app = App::new();
        app.add_plugins(RandomTickIndexPlugin)
            .insert_resource(RandomTickIndex::new(table));
        let pos = SectionPos::new(5, 0, -3);
        let section = app.world_mut().spawn((pos, EntityTicking)).id();
        let grass_section = ChunkBlocks::new(SectionStates::homogeneous(grass));
        let land = |app: &mut App, blocks: &ChunkBlocks| {
            app.world_mut().resource_mut::<RandomTickIndex>().insert(
                pos,
                &PalettedSection(&blocks.0.0),
                true,
            );
        };

        land(&mut app, &grass_section);
        assert_eq!(listed(&mut app), [pos], "a landed section of grass");

        land(&mut app, &air_section());
        assert!(listed(&mut app).is_empty(), "replaced by air");

        land(&mut app, &grass_section);
        app.world_mut()
            .entity_mut(section)
            .remove::<EntityTicking>();
        assert!(listed(&mut app).is_empty(), "out of the random-ticked area");
        assert!(
            listing(&mut app).columns.is_empty(),
            "no precipitation either"
        );

        app.world_mut().entity_mut(section).insert(EntityTicking);
        assert_eq!(listed(&mut app), [pos], "back in the area");
        assert_eq!(listing(&mut app).columns, [ColumnPos::from(pos)]);

        app.world_mut()
            .resource_mut::<RandomTickIndex>()
            .remove(pos);
        assert!(listed(&mut app).is_empty(), "unloaded");
        assert!(
            app.world().resource::<RandomTickIndex>().tiles.is_empty(),
            "an unloaded section leaves no tile behind"
        );
    }
}
