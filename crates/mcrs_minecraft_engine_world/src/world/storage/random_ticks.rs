use bevy_app::{App, Plugin};
use bevy_ecs::lifecycle::{Insert, Remove};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Query, ResMut};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_simulation::{
    RandomTickTable, SectionStreams, Spread, TickingPositions, TickingSectionMut, block_tile,
};
use mcrs_minecraft_volume::{PaletteBuffer, SectionBlocks, SectionBlocksMut};
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::world::storage::block_tile::BlockTile;
use crate::world::storage::entity_ticking::EntityTicking;
use crate::world::storage::indexed_section::IndexedSection;
use crate::world::storage::random_listing::RandomListing;
use crate::world::storage::tile_batch::TileBatch;

pub struct RandomTickIndexPlugin;

impl Plugin for RandomTickIndexPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(start_random_ticks)
            .add_observer(stop_random_ticks);
    }
}

#[derive(Resource, Debug)]
pub struct RandomTickIndex {
    table: RandomTickTable,
    tiles: FxHashMap<ColumnPos, BlockTile>,
    palette: PaletteBuffer,
    order: Vec<ColumnPos>,
    batches: Vec<TileBatch>,
}

fn tile_of(pos: SectionPos) -> ColumnPos {
    block_tile(ColumnPos::from(pos))
}

impl RandomTickIndex {
    #[must_use]
    pub fn new(table: RandomTickTable) -> Self {
        Self {
            table,
            tiles: FxHashMap::default(),
            palette: PaletteBuffer::default(),
            order: Vec::new(),
            batches: Vec::new(),
        }
    }

    pub fn insert<S: SectionBlocks>(&mut self, pos: SectionPos, blocks: &S, random_ticked: bool) {
        let mut ticking = TickingPositions::default();
        ticking.rebuild(blocks, &self.table, &mut self.palette);
        self.tiles
            .entry(tile_of(pos))
            .or_default()
            .upsert(IndexedSection {
                pos,
                random_ticked,
                ticking,
            });
    }

    pub(crate) fn remove(&mut self, pos: SectionPos) {
        let tile_pos = tile_of(pos);
        let Some(tile) = self.tiles.get_mut(&tile_pos) else {
            return;
        };
        tile.remove(pos);
        if tile.is_empty() {
            self.tiles.remove(&tile_pos);
        }
    }

    fn set_random_ticked(&mut self, pos: SectionPos, random_ticked: bool) {
        if let Some(tile) = self.tiles.get_mut(&tile_of(pos)) {
            tile.set_random_ticked(pos, random_ticked);
        }
    }

    #[cfg(test)]
    fn ticking(&self, pos: SectionPos) -> &[u16] {
        self.tiles
            .get(&tile_of(pos))
            .map_or(&[], |tile| tile.ticking(pos))
    }

    pub(crate) fn section_mut<'a, S: SectionBlocksMut>(
        &'a mut self,
        pos: SectionPos,
        blocks: S,
    ) -> TickingSectionMut<'a, S> {
        match self.tiles.get_mut(&tile_of(pos)) {
            Some(tile) => tile.section_mut(pos, blocks, &self.table),
            None => TickingSectionMut::untracked(blocks),
        }
    }

    pub fn draw<S: Spread>(
        &mut self,
        owns: impl Fn(ColumnPos) -> bool + Sync,
        speed: u32,
        streams: SectionStreams,
        out: &mut RandomListing,
        spread: &S,
    ) {
        let Self {
            tiles,
            order,
            batches,
            ..
        } = self;
        order.clear();
        order.extend(
            tiles
                .iter()
                .filter(|(_, tile)| tile.is_random_ticked())
                .map(|(pos, _)| *pos),
        );
        order.sort_unstable();
        TileBatch::split(batches, order.len());
        spread.spread(batches, &|batch: &mut TileBatch| {
            batch.draw_tiles(order, tiles, &owns, speed, streams);
        });
        out.hits.clear();
        out.columns.clear();
        for batch in batches {
            batch.drain_into(out);
            for tile_pos in batch.drain_stale() {
                if let Some(tile) = tiles.get_mut(&tile_pos) {
                    tile.prune();
                }
            }
        }
    }
}

fn start_random_ticks(
    insert: On<Insert<EntityTicking>>,
    sections: Query<&SectionPos>,
    index: Option<ResMut<RandomTickIndex>>,
) {
    if let (Some(mut index), Ok(pos)) = (index, sections.get(insert.event().entity)) {
        index.set_random_ticked(*pos, true);
    }
}

fn stop_random_ticks(
    remove: On<Remove<EntityTicking>>,
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
    use mcrs_minecraft_simulation::{
        GameTick, PRECIPITATION_TILE, RandomTicks, Serial, StreamSeed,
    };
    use mcrs_minecraft_volume::PalettedSection;
    use mcrs_voxel::BlockStateId;

    fn draw(index: &mut RandomTickIndex, owns: impl Fn(ColumnPos) -> bool + Sync) -> RandomListing {
        let mut out = RandomListing::default();
        let streams = StreamSeed::new(0, "minecraft:overworld").sections(GameTick(0));
        index.draw(owns, 1, streams, &mut out, &Serial);
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

    struct Reversed;

    impl Spread for Reversed {
        fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R) {
            std::thread::scope(|scope| {
                for item in items.iter_mut().rev() {
                    scope.spawn(move || run(item));
                }
            });
        }
    }

    #[test]
    fn tiles_drawn_on_threads_give_the_listing_of_one_thread() {
        let grass = BlockStateId::checked(1, 2).expect("a state past air");
        let mut index = RandomTickIndex::new(RandomTickTable::new([grass], |_| RandomTicks::BLOCK));
        let grass_section = ChunkBlocks::new(SectionStates::homogeneous(grass));
        for x in -4..4 {
            for z in -4..4 {
                index.insert(
                    SectionPos::new(x * PRECIPITATION_TILE, 0, z * PRECIPITATION_TILE + 1),
                    &PalettedSection(&grass_section.0.0),
                    true,
                );
            }
        }
        let streams = StreamSeed::new(7, "minecraft:overworld").sections(GameTick(3));
        let mut serial = RandomListing::default();
        let mut threads = RandomListing::default();
        index.draw(|_| true, 3, streams, &mut serial, &Serial);
        index.draw(|_| true, 3, streams, &mut threads, &Reversed);

        assert_eq!(serial.hits.len(), 3 * 64, "each pick in grass hits");
        assert_eq!(format!("{serial:?}"), format!("{threads:?}"));
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
