//! The positions that tick randomly in each loaded section of a dimension, by block tile.

use bevy_app::{App, Plugin};
use bevy_ecs::lifecycle::{Insert, Remove};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Has, Query, ResMut};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_simulation::{
    ListedSection, RandomTickTable, TickingPositions, TickingSectionMut, block_tile,
};
use mcrs_minecraft_volume::{PaletteBuffer, PalettedSection, SectionBlocks, SectionBlocksMut};
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::palette::ChunkBlocks;
use crate::world::lifecycle::simulation::EntityTicking;

/// Keeps the [`RandomTickIndex`] of a dimension world with the sections that land, unload and
/// start or stop random ticks. Block writes keep it through
/// [`WorldViewMut`](crate::world::storage::view::WorldViewMut).
pub struct RandomTickIndexPlugin;

impl Plugin for RandomTickIndexPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(index_landed_section)
            .add_observer(forget_unloaded_section)
            .add_observer(start_random_ticks)
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
}

#[derive(Debug, Default)]
struct BlockTile {
    /// Sorted by [`order`].
    sections: Vec<IndexedSection>,
    /// Slots of `sections`, sorted: each random-ticked section that holds a ticking position, and
    /// possibly some that no longer do.
    listed: Vec<u16>,
}

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

fn order(pos: SectionPos) -> (i32, i32, i32) {
    (pos.x, pos.z, pos.y)
}

fn tile_of(pos: SectionPos) -> ColumnPos {
    block_tile(ColumnPos::from(pos))
}

impl BlockTile {
    fn slot(&self, pos: SectionPos) -> Result<usize, usize> {
        self.sections
            .binary_search_by_key(&order(pos), |section| order(section.pos))
    }

    fn relist(&mut self) {
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

    pub fn remove(&mut self, pos: SectionPos) {
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

    pub fn set_random_ticked(&mut self, pos: SectionPos, random_ticked: bool) {
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
    }

    /// The local indices of the ticking positions of the section at `pos`.
    #[must_use]
    pub fn ticking(&self, pos: SectionPos) -> &[u16] {
        self.tiles
            .get(&tile_of(pos))
            .and_then(|tile| tile.sections.get(tile.slot(pos).ok()?))
            .map_or(&[], |section| section.ticking.as_slice())
    }

    /// A handle that writes `blocks`, the section at `pos`, and keeps its ticking positions.
    pub fn section_mut<'a, S: SectionBlocksMut>(
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

    /// Replaces `out` with the random-ticked sections that hold a ticking position, block tile
    /// by block tile in the order of the tiles, and in each tile in the order of the sections.
    pub fn listed(&mut self, out: &mut Vec<ListedSection>) {
        out.clear();
        self.order.clear();
        self.order.extend(
            self.tiles
                .iter()
                .filter(|(_, tile)| !tile.listed.is_empty())
                .map(|(pos, _)| *pos),
        );
        self.order.sort_unstable();
        for tile_pos in &self.order {
            let Some(tile) = self.tiles.get_mut(tile_pos) else {
                continue;
            };
            let BlockTile { sections, listed } = tile;
            listed.retain(|&slot| {
                sections
                    .get(usize::from(slot))
                    .is_some_and(IndexedSection::is_listed)
            });
            out.extend(
                listed
                    .iter()
                    .filter_map(|&slot| sections.get(usize::from(slot)))
                    .map(|section| ListedSection {
                        section: section.pos,
                        // At most 4096 positions.
                        ticking: section.ticking.as_slice().len() as u16,
                    }),
            );
        }
    }
}

fn index_landed_section(
    insert: On<Insert, ChunkBlocks>,
    sections: Query<(&SectionPos, &ChunkBlocks, Has<EntityTicking>)>,
    index: Option<ResMut<RandomTickIndex>>,
) {
    let (Some(mut index), Ok((pos, blocks, random_ticked))) =
        (index, sections.get(insert.event().entity))
    else {
        return;
    };
    index.insert(*pos, &PalettedSection(&blocks.0.0), random_ticked);
}

fn forget_unloaded_section(
    remove: On<Remove, ChunkBlocks>,
    sections: Query<&SectionPos>,
    index: Option<ResMut<RandomTickIndex>>,
) {
    if let (Some(mut index), Ok(pos)) = (index, sections.get(remove.event().entity)) {
        index.remove(*pos);
    }
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
    use crate::palette::{SectionStates, air_section};
    use mcrs_minecraft_simulation::RandomTicks;
    use mcrs_voxel::BlockStateId;

    fn listed(app: &mut App) -> Vec<SectionPos> {
        let mut out = Vec::new();
        app.world_mut()
            .resource_mut::<RandomTickIndex>()
            .listed(&mut out);
        out.iter().map(|listed| listed.section).collect()
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
        let grass_section = || ChunkBlocks::new(SectionStates::homogeneous(grass));

        app.world_mut().entity_mut(section).insert(grass_section());
        assert_eq!(listed(&mut app), [pos], "a landed section of grass");

        app.world_mut().entity_mut(section).insert(air_section());
        assert!(listed(&mut app).is_empty(), "replaced by air");

        app.world_mut()
            .entity_mut(section)
            .insert(grass_section())
            .remove::<EntityTicking>();
        assert!(listed(&mut app).is_empty(), "out of the random-ticked area");

        app.world_mut().entity_mut(section).insert(EntityTicking);
        assert_eq!(listed(&mut app), [pos], "back in the area");

        app.world_mut().despawn(section);
        assert!(listed(&mut app).is_empty(), "unloaded");
        assert!(
            app.world().resource::<RandomTickIndex>().tiles.is_empty(),
            "an unloaded section leaves no tile behind"
        );
    }
}
