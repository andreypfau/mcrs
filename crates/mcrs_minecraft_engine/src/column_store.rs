use std::marker::PhantomData;
use std::vec;

use bevy_app::{App, FixedUpdate, Last, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::prelude::{on_message, resource_exists};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_engine_store::{ChunkVersion, StoreInput, StoreOutput, VersionedChunk};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::owned_columns::{self, OwnedColumns};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{ColumnCoolSet, SectionEntitySet};
use mcrs_minecraft_engine_world::world::light_gate::ColumnLightSettled;
use mcrs_minecraft_engine_world::world::storage::chunk_spawn_set::ChunkSpawnSet;
use mcrs_minecraft_engine_world::world::storage::column::ColumnLifecycleSet;
use mcrs_minecraft_engine_world::world::storage::section_stage::SectionStageChanged;
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;

use crate::advance_production::advance_production;
use crate::cold_columns::{
    SaveRequest, columns_cooling, mark_changed_block_entities, mark_changed_sections,
    mark_final_light, save_all_loaded_columns, save_dirty_loaded_columns, write_cold_columns,
};
use crate::column_asks::{
    ColumnAnswer, ColumnAsk, ColumnHold, ColumnOrigin, Hold, OtherColumns, OwnerReply,
    answer_column_asks, column_asks_pending,
};
use crate::column_light::ColdStageLandSet;
use crate::column_production::ColumnProduction;
use crate::column_store_mail::ColumnStoreMail;
use crate::column_streams::{ColumnStreamSet, land_viewed_columns, report_player_views};
use crate::column_tick_holder::ColumnTickHolder;
use crate::heightmap::update_column_heightmaps;
use crate::hot_ticket::{raise_hot_ticket, release_hot_ticket, track_hot_tickets};
use crate::stored_columns_landing::{
    PendingColumnFacts, attach_column_facts, column_facts_pending, fetch_loading_columns,
    land_stored_columns,
};
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

pub type ColumnStoreInput = StoreInput<ColumnPos, StoredColumn>;

pub type ColumnStoreOutput = StoreOutput<ColumnPos, StoredColumn>;

pub struct ColumnStorePlugin<T> {
    owned: Option<OwnedColumns>,
    ticks: PhantomData<fn() -> T>,
}

impl<T> ColumnStorePlugin<T> {
    #[must_use]
    pub fn new(owned: Option<OwnedColumns>) -> Self {
        Self {
            owned,
            ticks: PhantomData,
        }
    }
}

#[derive(Resource, Default)]
pub struct ColumnStoreLink {
    owned: Option<OwnedColumns>,
    sent: Vec<ColumnStoreInput>,
    answers: Vec<ColumnStoreOutput>,
    written: ChunkVersion,
    pub(crate) reads: FxHashMap<ColumnPos, ColumnRead>,
    pub(crate) viewed: Vec<ViewedColumn>,
    pub(crate) fetched: Vec<FetchedColumn>,
    written_columns: FxHashMap<ColumnPos, ChunkVersion>,
    others: OtherColumns,
    asked: Vec<ColumnAsk>,
    out_asks: Vec<ColumnAsk>,
    out_answers: Vec<ColumnAnswer>,
    out_holds: Vec<ColumnHold>,
    generation: Generation,
    generation_asks: Vec<ColumnPos>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Generation {
    #[default]
    Off,
    World,
}

pub(crate) struct ColumnRead {
    pub(crate) sections: Vec<Entity>,
    pub(crate) produced: Produced,
    pub(crate) at_least: ChunkVersion,
    pub(crate) from: ReadFrom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadFrom {
    Store,
    Generation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Produced {
    No,
    Yes,
}

pub(crate) struct ViewedColumn {
    pub(crate) column: ColumnPos,
    pub(crate) stored: StoredColumn,
    pub(crate) delivery: Delivery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    Version,
    Light,
}

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
            ..Self::default()
        }
    }

    pub fn generate_through_world(&mut self) {
        self.generation = Generation::World;
    }

    #[must_use]
    pub fn generates(&self) -> bool {
        self.generation == Generation::World
    }

    pub(crate) fn take_generation_asks(&mut self, out: &mut Vec<ColumnPos>) {
        out.append(&mut self.generation_asks);
    }

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

    pub(crate) fn has_asks(&self) -> bool {
        !self.asked.is_empty()
    }

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

    pub(crate) fn take_mail(&mut self, mail: &mut ColumnStoreMail) {
        mail.asks.append(&mut self.out_asks);
        mail.answers.append(&mut self.out_answers);
        mail.holds.append(&mut self.out_holds);
    }

    pub(crate) fn last_written(&self, column: ColumnPos) -> Option<ChunkVersion> {
        self.written_columns.get(&column).copied()
    }

    pub(crate) fn take_sent(&mut self, out: &mut Vec<ColumnStoreInput>) {
        out.append(&mut self.sent);
    }

    pub(crate) fn receive(&mut self, answers: Vec<ColumnStoreOutput>) {
        self.answers.extend(answers);
    }

    pub(crate) fn put(&mut self, column: ColumnPos, chunk: StoredColumn) {
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
        if let Some(read) = self.reads.get_mut(&column) {
            read.at_least = self.written;
            read.from = ReadFrom::Store;
            self.sent.push(StoreInput::Get { key: column });
        }
    }

    pub(crate) fn put_lit(&mut self, column: ColumnPos, chunk: VersionedChunk<StoredColumn>) {
        self.sent.push(StoreInput::Put { key: column, chunk });
    }

    fn read(&mut self, column: ColumnPos) -> &mut ColumnRead {
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
                _ => {
                    self.sent.push(StoreInput::Get { key: column });
                    ReadFrom::Store
                }
            };
            ColumnRead {
                sections: Vec::new(),
                produced: Produced::No,
                at_least: ChunkVersion::default(),
                from,
            }
        })
    }

    pub(crate) fn take(&mut self, column: ColumnPos, section: Entity) {
        self.read(column).sections.push(section);
    }

    pub(crate) fn fetch(&mut self, column: ColumnPos) {
        self.read(column).produced = Produced::Yes;
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

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoredColumnLandSet;

impl<T: ColumnTickHolder> Plugin for ColumnStorePlugin<T> {
    fn build(&self, app: &mut App) {
        app.insert_resource(ColumnStoreLink::new(self.owned));
        app.init_resource::<ColumnProduction>();
        app.init_resource::<PendingColumnFacts>();
        app.configure_sets(
            FixedUpdate,
            ColdStageLandSet
                .in_set(TickSet::DomainInput)
                .after(StoredColumnLandSet),
        );
        app.add_observer(raise_hot_ticket);
        app.add_observer(release_hot_ticket);
        app.add_systems(
            FixedUpdate,
            (
                land_stored_columns
                    .in_set(TickSet::DomainInput)
                    .in_set(StoredColumnLandSet),
                (
                    mark_changed_sections.run_if(on_message::<BlockPlaced>),
                    mark_changed_block_entities,
                    write_cold_columns::<T>.run_if(columns_cooling),
                )
                    .chain()
                    .in_set(ColumnCoolSet),
                fetch_loading_columns
                    .run_if(on_message::<SectionStageChanged>)
                    .in_set(ChunkSpawnSet)
                    .after(SectionEntitySet),
                attach_column_facts
                    .run_if(column_facts_pending)
                    .in_set(TickSet::Project)
                    .after(ColumnLifecycleSet)
                    .before(ColumnCoolSet)
                    .before(ColumnStreamSet),
                mark_final_light
                    .run_if(on_message::<ColumnLightSettled>)
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
            app.add_systems(
                Last,
                answer_column_asks
                    .run_if(column_asks_pending)
                    .after(update_column_heightmaps),
            );
        }
        app.add_systems(
            Last,
            (
                save_dirty_loaded_columns::<T>.run_if(resource_exists::<SaveRequest>),
                save_all_loaded_columns::<T>.run_if(resource_exists::<SaveRequest>),
            )
                .chain()
                .after(update_column_heightmaps),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    #[test]
    fn a_column_never_written_here_asks_the_disk_before_the_world_generator() {
        let mut link = ColumnStoreLink::new(None);
        link.generate_through_world();
        let column = ColumnPos::new(3, 4);
        link.take(column, Entity::PLACEHOLDER);

        let mut asks = Vec::new();
        link.take_generation_asks(&mut asks);
        assert!(asks.is_empty(), "the generator waits for a missing answer");
        assert!(
            matches!(link.sent.first(), Some(StoreInput::Get { key }) if *key == column),
            "the data layer is asked first"
        );
    }
}
