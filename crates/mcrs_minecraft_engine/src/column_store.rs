use std::collections::vec_deque::Drain;
use std::vec;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_engine_link::{Link, Placement};
use mcrs_engine_store::{
    ChunkVersion, NoDisk, StoreInput, StoreOutput, StoreStage, VersionedChunk,
};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{
    ChunkSpawnSet, ColumnCoolSet, SectionEntitySet,
};
use mcrs_minecraft_engine_world::world::storage::column::ColumnLifecycleSet;
use mcrs_voxel::ColumnPos;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::cold_columns::{mark_changed_sections, mark_final_light, write_cold_columns};
use crate::column_view::ColumnViewSet;
use crate::hot_ticket::{HotColumns, raise_hot_ticket, release_hot_ticket, track_hot_tickets};
use crate::stored_column::StoredColumn;
use crate::stored_columns_landing::{fetch_loading_columns, land_stored_columns};

/// The disk under the data layer of a dimension. The data layer lives in memory only for now.
pub type ColumnDisk = NoDisk<ColumnPos, StoredColumn>;

/// The data layer of a dimension world, at the far end of a link: the last version of each
/// column that went cold.
///
/// With it, only the hot columns stay in the dimension: the columns that a player simulates,
/// and the columns that a view waits on while they are generated and lit. A column that stops
/// being loaded writes its last version here. A column that a ticket loads again is taken from
/// here before it is generated, and a view gets a cold column from here.
pub struct ColumnStorePlugin {
    pub placement: Placement,
}

/// The near end of the link to the data layer of a dimension world.
#[derive(Resource)]
pub struct ColumnStoreLink {
    link: Link<StoreStage<ColumnDisk>>,
    /// The version of the last column the world wrote. The world is the only writer.
    written: ChunkVersion,
    /// The columns that wait for an answer of the data layer.
    pub(crate) reads: FxHashMap<ColumnPos, ColumnRead>,
    /// The answers that views wait on, for the view to take.
    pub(crate) viewed: Vec<ViewedColumn>,
    /// Every column the world wrote. The data layer has no disk under it, so these are the
    /// only columns it can hold, and a request for any other one is not worth a round trip.
    written_columns: FxHashSet<ColumnPos>,
}

pub(crate) struct ColumnRead {
    pub(crate) dim: Entity,
    /// The sections of the dimension that wait to take the column back.
    pub(crate) sections: Vec<Entity>,
    pub(crate) viewed: Viewed,
    /// An answer older than this version was asked for before the world wrote a later one.
    pub(crate) at_least: ChunkVersion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Viewed {
    No,
    Yes,
}

/// The answer of the data layer for a column that a view waits on. `None` when the data layer
/// holds no version of it.
pub(crate) struct ViewedColumn {
    pub(crate) dim: Entity,
    pub(crate) column: ColumnPos,
    pub(crate) stored: Option<StoredColumn>,
}

impl ColumnStoreLink {
    #[must_use]
    pub fn new(placement: Placement) -> Self {
        Self {
            link: Link::new(StoreStage::new(ColumnDisk::new()), placement),
            written: ChunkVersion::default(),
            reads: FxHashMap::default(),
            viewed: Vec::new(),
            written_columns: FxHashSet::default(),
        }
    }

    /// Whether the data layer may hold a version of `column`.
    pub(crate) fn may_hold(&self, column: ColumnPos) -> bool {
        self.written_columns.contains(&column)
    }

    /// Waits until the data layer has answered every request sent so far, and lands none of it.
    pub async fn answered(&mut self) {
        self.link.answered().await;
    }

    pub(crate) fn put(&mut self, column: ColumnPos, chunk: StoredColumn) {
        self.written = self.written.next();
        self.written_columns.insert(column);
        self.link.send(StoreInput::Put {
            key: column,
            chunk: VersionedChunk {
                version: self.written,
                chunk,
            },
        });
        // The answer to the read in flight is older than this version, so it is asked again.
        if let Some(read) = self.reads.get_mut(&column) {
            read.at_least = self.written;
            self.link.send(StoreInput::Get { key: column });
        }
    }

    fn read(&mut self, dim: Entity, column: ColumnPos) -> &mut ColumnRead {
        self.reads.entry(column).or_insert_with(|| {
            self.link.send(StoreInput::Get { key: column });
            ColumnRead {
                dim,
                sections: Vec::new(),
                viewed: Viewed::No,
                at_least: ChunkVersion::default(),
            }
        })
    }

    /// Asks for `column` on behalf of `section`, once for all that wait on it.
    pub(crate) fn take(&mut self, dim: Entity, column: ColumnPos, section: Entity) {
        self.read(dim, column).sections.push(section);
    }

    /// Asks for `column` on behalf of the views that wait on it, once for all that wait on it.
    pub(crate) fn view(&mut self, dim: Entity, column: ColumnPos) {
        self.read(dim, column).viewed = Viewed::Yes;
    }

    pub(crate) fn outputs(&mut self) -> Drain<'_, StoreOutput<ColumnPos, StoredColumn>> {
        self.link.outputs()
    }

    pub(crate) fn take_viewed(&mut self) -> vec::Drain<'_, ViewedColumn> {
        self.viewed.drain(..)
    }
}

/// Where the sections that the data layer handed back land, at the start of the tick. Other
/// sources of section content land after it, so the order of the landings is fixed.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoredColumnLandSet;

impl Plugin for ColumnStorePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ColumnStoreLink::new(self.placement));
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
                    .before(ColumnViewSet),
            ),
        );
    }
}
