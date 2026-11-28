use core::ops::Range;
use std::vec::Drain;

use mcrs_minecraft_simulation::{PRECIPITATION_TILE, RandomPick, SectionStreams};
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;

use crate::world::storage::block_tile::{BlockTile, Listing};
use crate::world::storage::random_listing::RandomListing;

const TILES_IN_BATCH: usize = 32;

#[derive(Debug, Default)]
pub(crate) struct TileBatch {
    tiles: Range<usize>,
    drawn: Vec<u16>,
    picks: Vec<RandomPick>,
    pick_slots: Vec<u16>,
    listing: RandomListing,
    stale: Vec<ColumnPos>,
}

impl TileBatch {
    pub(crate) fn split(batches: &mut Vec<Self>, tile_count: usize) {
        batches.resize_with(tile_count.div_ceil(TILES_IN_BATCH), Self::default);
        for (index, batch) in batches.iter_mut().enumerate() {
            batch.tiles = index * TILES_IN_BATCH..tile_count.min((index + 1) * TILES_IN_BATCH);
        }
    }

    pub(crate) fn draw_tiles(
        &mut self,
        order: &[ColumnPos],
        tiles: &FxHashMap<ColumnPos, BlockTile>,
        owns: impl Fn(ColumnPos) -> bool,
        speed: u32,
        streams: SectionStreams,
    ) {
        for tile_pos in order.get(self.tiles.clone()).unwrap_or_default() {
            if let Some(tile) = tiles.get(tile_pos) {
                self.draw(*tile_pos, tile, &owns, speed, streams);
            }
        }
    }

    pub(crate) fn drain_into(&mut self, out: &mut RandomListing) {
        out.hits.append(&mut self.listing.hits);
        out.columns.append(&mut self.listing.columns);
    }

    pub(crate) fn drain_stale(&mut self) -> Drain<'_, ColumnPos> {
        self.stale.drain(..)
    }

    fn draw(
        &mut self,
        tile_pos: ColumnPos,
        tile: &BlockTile,
        owns: impl Fn(ColumnPos) -> bool,
        speed: u32,
        streams: SectionStreams,
    ) {
        let Self {
            drawn,
            picks,
            pick_slots,
            listing,
            stale,
            ..
        } = self;
        let column_pos = |column: i32| {
            ColumnPos::new(
                tile_pos.x * PRECIPITATION_TILE + column / PRECIPITATION_TILE,
                tile_pos.z * PRECIPITATION_TILE + column % PRECIPITATION_TILE,
            )
        };
        let owned = tile.owned_columns(|column| owns(column_pos(column)));
        if tile.drawn_slots(drawn) == Listing::Stale {
            stale.push(tile_pos);
        }
        for &slot in &*drawn {
            tile.draw_picks(slot, owned, speed, streams, picks);
            pick_slots.resize(picks.len(), slot);
        }
        for (slot, pick) in pick_slots.drain(..).zip(picks.drain(..)) {
            tile.extend_hits(slot, pick, &mut listing.hits);
        }
        listing.columns.extend(
            (0..PRECIPITATION_TILE * PRECIPITATION_TILE)
                .filter(|&column| owned & 1 << column != 0)
                .map(column_pos),
        );
    }
}
