use std::vec;

use bevy_app::{App, FixedUpdate, Last, Plugin};
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

use crate::advance_production::advance_production;
use crate::cell_mail::CellMail;
use crate::cold_columns::{mark_changed_sections, mark_final_light, write_cold_columns};
use crate::column_asks::{
    ColumnAnswer, ColumnAsk, ColumnHold, ColumnOrigin, Hold, OtherColumns, OwnerReply,
    answer_column_asks,
};
use crate::column_generation::GenerationAnswer;
use crate::column_light::ColdStageLandSet;
use crate::column_production::ColumnProduction;
use crate::column_streams::{ColumnStreamSet, land_viewed_columns, report_player_views};
use crate::heightmap::update_column_heightmaps;
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
    /// The columns of the other cells of the world that this cell can read.
    others: OtherColumns,
    /// The asks of the other cells for columns of this cell, until the owner answers them.
    asked: Vec<ColumnAsk>,
    /// What this tick hands the other cells: asks, answers and reports of columns.
    out_asks: Vec<ColumnAsk>,
    out_answers: Vec<ColumnAnswer>,
    out_holds: Vec<ColumnHold>,
    generation: Generation,
    /// The columns of this tick to generate, for the generation of the world.
    generation_asks: Vec<ColumnPos>,
}

/// Whether a cell generates the columns that no cell holds through the generation of its world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Generation {
    Off,
    World,
}

pub(crate) struct ColumnRead {
    pub(crate) dim: Entity,
    /// The sections of the dimension that wait to take the column back.
    pub(crate) sections: Vec<Entity>,
    pub(crate) produced: Produced,
    /// An answer older than this version was asked for before the world wrote a later one.
    pub(crate) at_least: ChunkVersion,
    pub(crate) from: ReadFrom,
}

/// Where a read waits for its column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadFrom {
    /// The owner, or the data layer: the answer is a version that the world already holds.
    Store,
    /// The generation of the world: a found column is new, and a missing one was dropped.
    Generation,
}

/// Whether the production of cold columns waits on a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Produced {
    No,
    Yes,
}

/// A final cold column for the views: a version that they wait on, or the new light of a
/// column that they hold.
pub(crate) struct ViewedColumn {
    pub(crate) dim: Entity,
    pub(crate) column: ColumnPos,
    pub(crate) stored: StoredColumn,
    pub(crate) delivery: Delivery,
}

/// What a [`ViewedColumn`] gives the views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    /// The version of the column, for the views that wait on it.
    Version,
    /// The light of the column only, as a change for the views that hold it.
    Light,
}

/// An answer of the data layer that the production of a cold column waits on.
pub(crate) struct FetchedColumn {
    pub(crate) column: ColumnPos,
    pub(crate) answer: ColumnStoreOutput,
    pub(crate) from: ReadFrom,
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
            others: OtherColumns::default(),
            asked: Vec::new(),
            out_asks: Vec::new(),
            out_answers: Vec::new(),
            out_holds: Vec::new(),
            generation: Generation::Off,
            generation_asks: Vec::new(),
        }
    }

    /// Generates the columns that no cell holds through the generation of the world, which the
    /// host runs for all cells of the world: the asks go in the output of a tick, and the
    /// answers land as answers of the data layer.
    pub fn generate_through_world(&mut self) {
        self.generation = Generation::World;
    }

    /// Whether the columns that no cell holds come from the generation of the world.
    pub(crate) fn generates(&self) -> bool {
        self.generation == Generation::World
    }

    /// Moves the asks of this tick for the generation of the world into `out`.
    pub(crate) fn take_generation_asks(&mut self, out: &mut Vec<ColumnPos>) {
        out.append(&mut self.generation_asks);
    }

    /// Keeps what the generation of the world made for the next landing.
    pub(crate) fn receive_generated(&mut self, answers: Vec<GenerationAnswer>) {
        self.answers
            .extend(answers.into_iter().map(|answer| match answer.stored {
                Some(chunk) => StoreOutput::Found {
                    key: answer.column,
                    chunk: VersionedChunk {
                        version: ChunkVersion::default(),
                        chunk,
                    },
                },
                None => StoreOutput::Missing { key: answer.column },
            }));
    }

    /// Sends the read of `column` on to the generation of the world, after the owner or the
    /// data layer did not hold the column.
    pub(crate) fn read_generated(&mut self, column: ColumnPos) -> bool {
        if !self.generates() {
            return false;
        }
        let Some(read) = self.reads.get_mut(&column) else {
            return false;
        };
        read.from = ReadFrom::Generation;
        self.generation_asks.push(column);
        true
    }

    /// Where this cell gets `column` when it does not hold it. The data layer has no disk under
    /// it, so the writes are the only columns that it holds.
    pub(crate) fn origin(&self, column: ColumnPos) -> ColumnOrigin {
        if !owned_columns::owns(self.owned.as_ref(), column) {
            return self.others.origin(column);
        }
        if self.written_columns.contains_key(&column) {
            ColumnOrigin::DataLayer
        } else {
            ColumnOrigin::Generation
        }
    }

    /// Whether no request, answer or ask waits in the link.
    pub(crate) fn is_idle(&self) -> bool {
        self.sent.is_empty()
            && self.answers.is_empty()
            && self.reads.is_empty()
            && self.viewed.is_empty()
            && self.fetched.is_empty()
            && self.asked.is_empty()
            && self.out_asks.is_empty()
            && self.out_answers.is_empty()
            && self.out_holds.is_empty()
            && self.generation_asks.is_empty()
    }

    /// Tells the other cells of a change of `hold.column`, when they can read it.
    pub(crate) fn report(&mut self, hold: ColumnHold) {
        if self
            .owned
            .is_some_and(|owned| owned.is_read_by_others(hold.column))
        {
            self.out_holds.push(hold);
        }
    }

    pub(crate) fn note_hold(&mut self, hold: ColumnHold) {
        self.others.note(hold);
    }

    pub(crate) fn receive_ask(&mut self, ask: ColumnAsk) {
        self.asked.push(ask);
    }

    /// Answers each ask that `reply` answers now, and keeps the others for a later tick.
    pub(crate) fn answer_asks(&mut self, mut reply: impl FnMut(ColumnPos) -> Option<OwnerReply>) {
        let out = &mut self.out_answers;
        self.asked.retain(|ask| match reply(ask.column) {
            Some(reply) => {
                out.push(ColumnAnswer {
                    to: ask.from,
                    column: ask.column,
                    reply,
                });
                false
            }
            None => true,
        });
    }

    /// Takes the answer of the owner of a column that this cell asked for: the column lands as
    /// an answer of the data layer. A column that the owner let go of is asked of the data
    /// layer when a cell wrote it, and is generated otherwise.
    pub(crate) fn receive_answer(&mut self, answer: ColumnAnswer) {
        let key = answer.column;
        match answer.reply {
            OwnerReply::Snapshot(chunk) => self.answers.push(StoreOutput::Found {
                key,
                chunk: VersionedChunk {
                    version: ChunkVersion::default(),
                    chunk: *chunk,
                },
            }),
            OwnerReply::NotHeld if self.others.is_written(key) => {
                self.sent.push(StoreInput::Get { key });
            }
            OwnerReply::NotHeld => {
                if !self.read_generated(key) {
                    self.answers.push(StoreOutput::Missing { key });
                }
            }
        }
    }

    /// Moves what this tick hands the other cells into `mail`.
    pub(crate) fn take_mail(&mut self, mail: &mut CellMail) {
        mail.asks.append(&mut self.out_asks);
        mail.answers.append(&mut self.out_answers);
        mail.holds.append(&mut self.out_holds);
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
        self.report(ColumnHold {
            column,
            hold: Hold::Written,
        });
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
            read.from = ReadFrom::Store;
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

    /// The read of `column`, sent to its owner when the owner holds it hot, to the generation of
    /// the world when no cell wrote it and the cell generates through its world, and to the data
    /// layer otherwise.
    fn read(&mut self, dim: Entity, column: ColumnPos) -> &mut ColumnRead {
        let origin = self.origin(column);
        self.reads.entry(column).or_insert_with(|| {
            let from = match (self.owned, origin, self.generation) {
                (Some(owned), ColumnOrigin::Owner, _) => {
                    self.out_asks.push(ColumnAsk {
                        from: owned.index(),
                        column,
                    });
                    ReadFrom::Store
                }
                (_, ColumnOrigin::Generation, Generation::World) => {
                    self.generation_asks.push(column);
                    ReadFrom::Generation
                }
                _ => {
                    self.sent.push(StoreInput::Get { key: column });
                    ReadFrom::Store
                }
            };
            ColumnRead {
                dim,
                sections: Vec::new(),
                produced: Produced::No,
                at_least: ChunkVersion::default(),
                from,
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
            ColdStageLandSet::Light
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
        if self.owned.is_some() {
            app.add_systems(Last, answer_column_asks.after(update_column_heightmaps));
        }
    }
}
