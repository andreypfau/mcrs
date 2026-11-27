use bevy_ecs::prelude::{Added, Query, Res, ResMut};
use mcrs_engine_cell::CellIndex;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnPosComponent};
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashSet;

use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::ColumnStoreLink;
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnAsk {
    pub(crate) from: CellIndex,
    pub(crate) column: ColumnPos,
}

#[derive(Clone, Debug)]
pub struct ColumnAnswer {
    pub(crate) to: CellIndex,
    pub(crate) column: ColumnPos,
    pub(crate) reply: OwnerReply,
}

#[derive(Clone, Debug)]
pub(crate) enum OwnerReply {
    Snapshot(Box<StoredColumn>),
    NotHeld,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColumnOrigin {
    Owner,
    DataLayer,
    Generation,
}

#[derive(Debug, Default)]
pub(crate) struct OtherColumns {
    hot: FxHashSet<ColumnPos>,
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

pub(crate) fn column_asks_pending(
    store: Res<ColumnStoreLink>,
    spawned: Query<(), Added<ColumnPosComponent>>,
) -> bool {
    store.has_asks() || !spawned.is_empty()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "a snapshot is boxed only for a column that another cell asks for, and it moves into the answer mail"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(4))]
pub(crate) fn answer_column_asks(
    mut store: ResMut<ColumnStoreLink>,
    owned: Res<OwnedColumns>,
    dims: Query<(&ColumnLoading, &ColumnIndex)>,
    snapshots: ColumnSnapshots,
    spawned: Query<&ColumnPosComponent, Added<ColumnPosComponent>>,
) {
    for &ColumnPosComponent(column) in &spawned {
        if owned.owns(column) {
            store.report(ColumnHold {
                column,
                hold: Hold::Hot,
            });
        }
    }
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
