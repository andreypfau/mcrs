use core::mem;

use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::TileBodies;

use crate::tile_edits::Arrival;
use crate::tile_edits::Departure;
use crate::tile_edits::TileBatch;

#[derive(Default)]
pub(crate) struct PartWork {
    incoming: Vec<TileBatch>,
    gone: Vec<Departure>,
    added: Vec<Arrival>,
    rows: Vec<GridRow>,
    tiles: Vec<TileWork>,
    filled: usize,
}

#[derive(Default)]
struct TileWork {
    tile: TileKey,
    bodies: TileBodies,
    reach: f32,
}

impl PartWork {
    pub(crate) fn accept(&mut self, source_index: usize, batch: &mut TileBatch) {
        if self.incoming.len() <= source_index {
            self.incoming
                .resize_with(source_index + 1, TileBatch::default);
        }
        if let Some(incoming) = self.incoming.get_mut(source_index) {
            mem::swap(incoming, batch);
        }
    }

    pub(crate) fn commit(&mut self, grid: &mut BodyGrid) {
        let filled = self.tiles.get_mut(..self.filled).unwrap_or_default();
        for work in filled {
            grid.swap(work.tile, &mut work.bodies);
            grid.widen(work.reach);
        }
        self.filled = 0;
    }

    pub(crate) fn build(&mut self, grid: &BodyGrid) {
        let Self {
            incoming,
            gone,
            added,
            rows,
            tiles,
            filled,
        } = self;
        gone.clear();
        added.clear();
        for batch in incoming.iter_mut() {
            gone.append(&mut batch.gone);
            added.append(&mut batch.added);
        }
        gone.sort_unstable();
        gone.dedup();
        added.sort_unstable_by_key(|arrival| arrival.tile);
        *filled = 0;
        let (mut gone, mut added) = (&gone[..], &added[..]);
        while let Some(tile) = next_tile(gone, added) {
            let left = gone.partition_point(|departure| departure.tile <= tile);
            let (here_gone, rest_gone) = gone.split_at(left);
            let arrived = added.partition_point(|arrival| arrival.tile <= tile);
            let (here_added, rest_added) = added.split_at(arrived);
            (gone, added) = (rest_gone, rest_added);
            rows.clear();
            let kept = (grid.tile(tile).rows().iter()).filter(|row| {
                here_gone
                    .binary_search_by_key(&row.key, |departure| departure.key)
                    .is_err()
            });
            rows.extend(kept);
            rows.extend(here_added.iter().map(|arrival| arrival.row));
            if tiles.len() <= *filled {
                tiles.push(TileWork::default());
            }
            if let Some(work) = tiles.get_mut(*filled) {
                work.tile = tile;
                work.reach = work.bodies.fill(tile, rows);
                *filled += 1;
            }
        }
    }
}

fn next_tile(gone: &[Departure], added: &[Arrival]) -> Option<TileKey> {
    match (gone.first(), added.first()) {
        (Some(departure), Some(arrival)) => Some(departure.tile.min(arrival.tile)),
        (Some(departure), None) => Some(departure.tile),
        (None, Some(arrival)) => Some(arrival.tile),
        (None, None) => None,
    }
}
