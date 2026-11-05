//! The reads of a column of another cell of the world. The owner of a column that holds it hot
//! may hold changes that it did not write to the data layer yet, so a cell asks the owner for
//! such a column and asks the data layer only for a column that is cold in its owner.
//!
//! Each cell tells the other cells when a column that they can read becomes hot in it, goes
//! cold, or is written. With these reports a cell sends each read to the owner, to the data
//! layer or to generation at once, in the tick that needs the column.

use bevy_ecs::prelude::{Added, Query, Res, ResMut};
use mcrs_engine_cell::CellIndex;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnPosComponent};
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashSet;

use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::ColumnStoreLink;
use crate::stored_column::StoredColumn;

/// A cell asks the owner of `column` for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnAsk {
    pub(crate) from: CellIndex,
    pub(crate) column: ColumnPos,
}

/// The owner of `column` answers the cell `to`.
#[derive(Clone, Debug)]
pub struct ColumnAnswer {
    pub(crate) to: CellIndex,
    pub(crate) column: ColumnPos,
    pub(crate) reply: OwnerReply,
}

#[derive(Clone, Debug)]
pub(crate) enum OwnerReply {
    /// The blocks of the column at the end of the tick of the answer. The changes of that tick
    /// also reach the halo, so the column misses none of them.
    Snapshot(Box<StoredColumn>),
    /// The owner does not hold the column any more.
    NotHeld,
}

/// A change of a column that other cells can read, in its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnHold {
    pub(crate) column: ColumnPos,
    pub(crate) hold: Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hold {
    Hot,
    Cold,
    Written,
}

/// Where a cell gets a column that it does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColumnOrigin {
    /// The owner holds the column hot.
    Owner,
    /// A cell wrote the column, and the owner does not hold it.
    DataLayer,
    /// No cell wrote the column, and the owner does not hold it.
    Generation,
}

/// What a cell knows of the columns of the other cells that it can read, from their reports.
#[derive(Debug, Default)]
pub(crate) struct OtherColumns {
    hot: FxHashSet<ColumnPos>,
    /// The data layer has no disk under it, so the writes are the only columns it holds.
    written: FxHashSet<ColumnPos>,
}

impl OtherColumns {
    pub(crate) fn note(&mut self, hold: ColumnHold) {
        match hold.hold {
            Hold::Hot => {
                self.hot.insert(hold.column);
            }
            Hold::Cold => {
                self.hot.remove(&hold.column);
            }
            Hold::Written => {
                self.written.insert(hold.column);
            }
        }
    }

    pub(crate) fn origin(&self, column: ColumnPos) -> ColumnOrigin {
        if self.hot.contains(&column) {
            ColumnOrigin::Owner
        } else if self.written.contains(&column) {
            ColumnOrigin::DataLayer
        } else {
            ColumnOrigin::Generation
        }
    }

    pub(crate) fn is_written(&self, column: ColumnPos) -> bool {
        self.written.contains(&column)
    }
}

/// Answers the asks of the other cells after every change of the tick: with the column when
/// its sections have landed, and with [`OwnerReply::NotHeld`] when no ticket loads it. An ask
/// for a column that still lands waits. It also reports each column that other cells can read
/// and that became hot.
pub(crate) fn answer_column_asks(
    mut store: ResMut<ColumnStoreLink>,
    owned: Res<OwnedColumns>,
    dims: Query<(&ColumnLoading, &ColumnIndex)>,
    snapshots: ColumnSnapshots,
    spawned: Query<&ColumnPosComponent, Added<ColumnPosComponent>>,
) {
    for &ColumnPosComponent(column) in &spawned {
        if owned.owns(column) && owned.is_read_by_others(column) {
            store.report(ColumnHold {
                column,
                hold: Hold::Hot,
            });
        }
    }
    // A dimension world holds one dimension.
    let dim = dims.iter().next();
    store.answer_asks(|column| {
        let Some((_, index)) = dim.filter(|(loading, _)| loading.is_column_loaded(column)) else {
            return Some(OwnerReply::NotHeld);
        };
        let slot = index.get(&column)?;
        snapshots
            .take_unlit(slot.entity)
            .map(|stored| OwnerReply::Snapshot(Box::new(stored)))
    });
}
