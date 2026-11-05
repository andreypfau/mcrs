use std::vec;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_engine_store::{ChunkVersion, NoDisk, StoreInput, StoreOutput, VersionedChunk};
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{
    ChunkSpawnSet, ColumnCoolSet, SectionEntitySet,
};
use mcrs_minecraft_engine_world::world::storage::column::ColumnLifecycleSet;
use mcrs_voxel::ColumnPos;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::cold_columns::{mark_changed_sections, mark_final_light, write_cold_columns};
use crate::column_production::{ColdStageLandSet, ColumnProduction, advance_production};
use crate::column_streams::{ColumnStreamSet, land_viewed_columns, report_player_views};
use crate::hot_ticket::{HotColumns, raise_hot_ticket, release_hot_ticket, track_hot_tickets};
use crate::stored_column::StoredColumn;
use crate::stored_columns_landing::{fetch_loading_columns, land_stored_columns};

/// The disk under the data layer of a dimension. The data layer lives in memory only for now.
pub type ColumnDisk = NoDisk<ColumnPos, StoredColumn>;

/// What a cell asks of the data layer of its world.
pub type ColumnStoreInput = StoreInput<ColumnPos, StoredColumn>;

/// What the data layer of a world answers a cell.
pub type ColumnStoreOutput = StoreOutput<ColumnPos, StoredColumn>;

/// The data layer of a dimension: the last version of each column that went cold. The
/// coordinator holds one data layer for each dimension, and every cell of the dimension reaches
/// it through the input and the output of its tick.
///
/// With it, only the hot columns stay in the dimension: the columns that a player simulates and
/// the rings that load with them. A column that stops being loaded writes its last version
/// here. A column that a ticket loads again is taken from here before it is generated. A view
/// gets every column that the dimension does not light from here, and the data layer generates
/// and lights a column that it does not hold with [`ColumnProduction`].
pub struct ColumnStorePlugin {
    /// The columns of the cell, in a dimension of several cells.
    pub owned: Option<OwnedColumns>,
}

/// The near end of the data layer of a dimension world: the requests of this tick go out in the
/// output of the tick, and the answers come in the input of a later tick.
#[derive(Resource)]
pub struct ColumnStoreLink {
    owned: Option<OwnedColumns>,
    /// The requests of this tick, in order.
    sent: Vec<ColumnStoreInput>,
    /// The answers that arrived and did not land yet, in the order of the requests.
    answers: Vec<ColumnStoreOutput>,
    /// The version of the last column this cell wrote. Only the owner of a column writes it.
    written: ChunkVersion,
    /// The columns that wait for an answer of the data layer.
    pub(crate) reads: FxHashMap<ColumnPos, ColumnRead>,
    /// The cold columns that views wait on, final, for the view to take.
    pub(crate) viewed: Vec<ViewedColumn>,
    /// The answers that the production of cold columns waits on.
    pub(crate) fetched: Vec<FetchedColumn>,
    /// The last version of every column this cell wrote. The data layer has no disk under it,
    /// so the writes are the only changed columns it holds, and a request for another column of
    /// this cell is not worth a round trip.
    written_columns: FxHashMap<ColumnPos, ChunkVersion>,
    /// The columns whose version the disk did not read. The world plays a generated column in
    /// their place, but never writes over the version on the disk, which may still be repaired.
    unreadable: FxHashSet<ColumnPos>,
}

pub(crate) struct ColumnRead {
    pub(crate) dim: Entity,
    /// The sections of the dimension that wait to take the column back.
    pub(crate) sections: Vec<Entity>,
    pub(crate) produced: Produced,
    /// An answer older than this version was asked for before the world wrote a later one.
    pub(crate) at_least: ChunkVersion,
}

/// Whether the production of cold columns waits on a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Produced {
    No,
    Yes,
}

/// A final cold column that views wait on.
pub(crate) struct ViewedColumn {
    pub(crate) dim: Entity,
    pub(crate) column: ColumnPos,
    pub(crate) stored: StoredColumn,
}

/// An answer of the data layer that the production of a cold column waits on.
pub(crate) struct FetchedColumn {
    pub(crate) column: ColumnPos,
    pub(crate) answer: ColumnStoreOutput,
}

impl ColumnStoreLink {
    #[must_use]
    pub fn new(owned: Option<OwnedColumns>) -> Self {
        Self {
            owned,
            sent: Vec::new(),
            answers: Vec::new(),
            written: ChunkVersion::default(),
            reads: FxHashMap::default(),
            viewed: Vec::new(),
            fetched: Vec::new(),
            written_columns: FxHashMap::default(),
            unreadable: FxHashSet::default(),
        }
    }

    /// Whether the data layer may hold a version of `column` that a cell wrote. Another cell
    /// writes the columns it owns, and only the data layer knows which.
    pub(crate) fn may_hold(&self, column: ColumnPos) -> bool {
        self.written_columns.contains_key(&column)
            || !owned_columns::owns(self.owned.as_ref(), column)
    }

    /// The last version of `column` that the world wrote.
    pub(crate) fn last_written(&self, column: ColumnPos) -> Option<ChunkVersion> {
        self.written_columns.get(&column).copied()
    }

    /// Moves the requests of this tick into `out`.
    pub(crate) fn take_sent(&mut self, out: &mut Vec<ColumnStoreInput>) {
        out.append(&mut self.sent);
    }

    /// Keeps answers of the data layer for the next landing.
    pub(crate) fn receive(&mut self, answers: Vec<ColumnStoreOutput>) {
        self.answers.extend(answers);
    }

    /// Keeps the disk version of `column` safe from every later write.
    pub(crate) fn mark_unreadable(&mut self, column: ColumnPos) {
        self.unreadable.insert(column);
    }

    /// Whether the world may write `column`: not when the disk holds a version it did not read.
    pub(crate) fn is_writable(&self, column: ColumnPos) -> bool {
        !self.unreadable.contains(&column)
    }

    pub(crate) fn put(&mut self, column: ColumnPos, chunk: StoredColumn) {
        if !self.is_writable(column) {
            return;
        }
        self.written = self.written.next();
        self.written_columns.insert(column, self.written);
        self.sent.push(StoreInput::Put {
            key: column,
            chunk: VersionedChunk {
                version: self.written,
                chunk,
            },
        });
        // The answer to the read in flight is older than this version, so it is asked again.
        if let Some(read) = self.reads.get_mut(&column) {
            read.at_least = self.written;
            self.sent.push(StoreInput::Get { key: column });
        }
    }

    /// Keeps the final light of a version the data layer already holds, or of a generated
    /// column at version zero. A later version that the world wrote meanwhile stays.
    pub(crate) fn put_lit(&mut self, column: ColumnPos, chunk: VersionedChunk<StoredColumn>) {
        if !self.is_writable(column) {
            return;
        }
        self.sent.push(StoreInput::Put { key: column, chunk });
    }

    fn read(&mut self, dim: Entity, column: ColumnPos) -> &mut ColumnRead {
        self.reads.entry(column).or_insert_with(|| {
            self.sent.push(StoreInput::Get { key: column });
            ColumnRead {
                dim,
                sections: Vec::new(),
                produced: Produced::No,
                at_least: ChunkVersion::default(),
            }
        })
    }

    /// Asks for `column` on behalf of `section`, once for all that wait on it.
    pub(crate) fn take(&mut self, dim: Entity, column: ColumnPos, section: Entity) {
        self.read(dim, column).sections.push(section);
    }

    /// Asks for `column` on behalf of the production of cold columns.
    pub(crate) fn fetch(&mut self, dim: Entity, column: ColumnPos) {
        self.read(dim, column).produced = Produced::Yes;
    }

    pub(crate) fn outputs(&mut self) -> vec::Drain<'_, ColumnStoreOutput> {
        self.answers.drain(..)
    }

    pub(crate) fn take_viewed(&mut self) -> vec::Drain<'_, ViewedColumn> {
        self.viewed.drain(..)
    }

    pub(crate) fn take_fetched(&mut self) -> vec::Drain<'_, FetchedColumn> {
        self.fetched.drain(..)
    }
}

/// Where the sections that the data layer handed back land, at the start of the tick. Other
/// sources of section content land after it, so the order of the landings is fixed.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoredColumnLandSet;

impl Plugin for ColumnStorePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ColumnStoreLink::new(self.owned));
        app.init_resource::<ColumnProduction>();
        app.configure_sets(
            FixedUpdate,
            (ColdStageLandSet::Generation, ColdStageLandSet::Light)
                .chain()
                .in_set(TickSet::DomainInput)
                .after(StoredColumnLandSet),
        );
        app.register_required_components::<ColumnLoading, HotColumns>();
        app.add_observer(raise_hot_ticket);
        app.add_observer(release_hot_ticket);
        app.add_systems(
            FixedUpdate,
            (
                land_stored_columns
                    .in_set(TickSet::DomainInput)
                    .in_set(StoredColumnLandSet),
                (mark_changed_sections, write_cold_columns)
                    .chain()
                    .in_set(ColumnCoolSet),
                fetch_loading_columns
                    .in_set(ChunkSpawnSet)
                    .after(SectionEntitySet),
                mark_final_light
                    .in_set(TickSet::Project)
                    .after(ColumnLifecycleSet)
                    .before(ColumnCoolSet),
                track_hot_tickets
                    .in_set(TickSet::Project)
                    .after(ColumnLifecycleSet)
                    .before(ColumnStreamSet),
                advance_production
                    .in_set(ColumnStreamSet)
                    .after(report_player_views)
                    .before(land_viewed_columns),
            ),
        );
    }
}
