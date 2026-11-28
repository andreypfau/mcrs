use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::GridRow;

pub(crate) const PARTS: usize = 64;

#[derive(Default)]
pub(crate) struct TileEdits {
    parts: Vec<TileBatch>,
}

#[derive(Default)]
pub(crate) struct TileBatch {
    pub(crate) gone: Vec<Departure>,
    pub(crate) added: Vec<Arrival>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Departure {
    pub(crate) tile: TileKey,
    pub(crate) key: EntityUuid,
}

#[derive(Clone, Copy)]
pub(crate) struct Arrival {
    pub(crate) tile: TileKey,
    pub(crate) row: GridRow,
}

#[derive(Clone, Copy)]
pub(crate) struct SlotChange {
    pub(crate) old: Option<GridRow>,
    pub(crate) new: Option<GridRow>,
}

impl TileEdits {
    pub(crate) fn push(&mut self, change: SlotChange) {
        if let Some(old) = change.old {
            self.leave(&old);
        }
        if let Some(row) = change.new {
            let tile = TileKey::at(row.position);
            if let Some(batch) = self.part(tile) {
                batch.added.push(Arrival { tile, row });
            }
        }
    }

    pub(crate) fn leave(&mut self, row: &GridRow) {
        let tile = TileKey::at(row.position);
        if let Some(batch) = self.part(tile) {
            batch.gone.push(Departure { tile, key: row.key });
        }
    }

    pub(crate) fn parts_mut(&mut self) -> &mut [TileBatch] {
        &mut self.parts
    }

    fn part(&mut self, tile: TileKey) -> Option<&mut TileBatch> {
        if self.parts.len() < PARTS {
            self.parts.resize_with(PARTS, TileBatch::default);
        }
        self.parts.get_mut(part_of(tile))
    }
}

impl TileBatch {
    pub(crate) fn is_empty(&self) -> bool {
        self.gone.is_empty() && self.added.is_empty()
    }
}

fn part_of(tile: TileKey) -> usize {
    let mixed = (tile.x.cast_unsigned().wrapping_mul(0x9E37_79B9))
        ^ (tile.z.cast_unsigned().wrapping_mul(0x85EB_CA6B))
        ^ (tile.y.cast_unsigned().wrapping_mul(0xC2B2_AE35));
    (mixed >> 26) as usize
}
