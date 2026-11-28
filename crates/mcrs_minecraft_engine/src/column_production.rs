use std::mem;
use std::vec;

use bevy_ecs::prelude::Resource;
use mcrs_engine_store::{ChunkVersion, StoreOutput, VersionedChunk};
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::voxel_update::BlockChange;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::cold_light_edit::ColdLightEdit;
use crate::column_asks::ColumnOrigin;
use crate::column_light::ColumnLight;
use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::{ColumnStoreLink, Delivery, ReadFrom, ViewedColumn};
use crate::wanted_column::{Ask, Changed, Content, Source, Wanted};
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

#[derive(Resource, Default)]
pub struct ColumnProduction {
    wanted: FxHashMap<ColumnPos, Wanted>,
    fetching: FxHashMap<ColumnPos, Source>,
    lit_world: FxHashSet<ColumnPos>,
    light_edits: Vec<ColdLightEdit>,
    light_awaits: Vec<ColumnPos>,
    lit: Vec<ColumnLight>,
    halo_changes: Vec<BlockChange>,
    views: Vec<ChunkTrackingView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lights {
    Yes,
    No,
}

impl ColumnProduction {
    pub fn take_light_edits(&mut self) -> vec::Drain<'_, ColdLightEdit> {
        self.light_edits.drain(..)
    }

    pub fn take_light_awaits(&mut self) -> vec::Drain<'_, ColumnPos> {
        self.light_awaits.drain(..)
    }

    #[must_use]
    pub fn has_light_work(&self) -> bool {
        !self.light_edits.is_empty() || !self.light_awaits.is_empty()
    }

    pub fn lit(&mut self, light: ColumnLight) {
        self.lit.push(light);
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.wanted.is_empty()
            && self.fetching.is_empty()
            && self.lit_world.is_empty()
            && self.light_edits.is_empty()
            && self.light_awaits.is_empty()
            && self.lit.is_empty()
            && self.halo_changes.is_empty()
    }

    pub(crate) fn take_halo_changes(&mut self) -> Vec<BlockChange> {
        mem::take(&mut self.halo_changes)
    }

    pub(crate) fn note_halo_changes(&mut self, changes: impl IntoIterator<Item = BlockChange>) {
        self.halo_changes.extend(changes);
    }

    pub(crate) fn want(&mut self, column: ColumnPos, store: &ColumnStoreLink) {
        self.wanted
            .entry(column)
            .or_insert_with(|| Wanted::new(Delivery::Version, store.last_written(column)))
            .delivery = Delivery::Version;
    }

    fn relight(&mut self, column: ColumnPos, store: &ColumnStoreLink) {
        self.wanted
            .entry(column)
            .or_insert_with(|| Wanted::new(Delivery::Light, store.last_written(column)));
    }

    fn fetch(
        &mut self,
        column: ColumnPos,
        loading: Option<&ColumnLoading>,
        store: &mut ColumnStoreLink,
    ) {
        if self.fetching.contains_key(&column) {
            return;
        }
        let source = if loading.is_some_and(|loading| loading.is_column_loaded(column)) {
            Source::Dimension
        } else if store.origin(column) != ColumnOrigin::Generation {
            store.fetch(column);
            Source::DataLayer(ChunkVersion::default())
        } else {
            if store.generates() {
                store.fetch(column);
            }
            Source::Generation
        };
        self.fetching.insert(column, source);
    }

    fn arrive(&mut self, column: ColumnPos, stored: StoredColumn, source: Source, lights: Lights) {
        self.fetching.remove(&column);
        if lights == Lights::Yes && self.views.iter().any(|view| view.reaches(column, 1)) {
            for (y, section) in (stored.min_section_y..).zip(&stored.sections) {
                self.light_edits.push(ColdLightEdit::Load {
                    pos: SectionPos::new(column.x, y, column.z),
                    blocks: section.blocks.clone(),
                });
            }
            self.lit_world.insert(column);
        }
        if let Some(wanted) = self.wanted.get_mut(&column)
            && wanted.content.is_none()
        {
            wanted.content = Some(Content { stored, source });
        }
    }

    pub(crate) fn screen(
        &mut self,
        views: &mut Vec<ChunkTrackingView>,
        rows: Option<&DimensionTypeConfig>,
    ) {
        if *views == self.views {
            views.clear();
            return;
        }
        mem::swap(&mut self.views, views);
        views.clear();
        let views = &self.views;
        self.wanted
            .retain(|column, _| views.iter().any(|view| view.reaches(*column, 0)));
        let Some(rows) = rows else {
            return;
        };
        let first = rows.min_y >> SectionPos::BITS;
        let count = i32::try_from(rows.section_count).unwrap_or(0);
        let light_edits = &mut self.light_edits;
        self.lit_world.retain(|column| {
            if views.iter().any(|view| view.reaches(*column, 1)) {
                return true;
            }
            for y in first..first.saturating_add(count) {
                light_edits.push(ColdLightEdit::Unload {
                    pos: SectionPos::new(column.x, y, column.z),
                });
            }
            false
        });
    }

    fn finish(
        &mut self,
        column: ColumnPos,
        light: Option<ColumnLight>,
        loading: Option<&ColumnLoading>,
        store: &mut ColumnStoreLink,
    ) {
        let Some(wanted) = self.wanted.get_mut(&column) else {
            return;
        };
        let Some(content) = &wanted.content else {
            return;
        };
        let loaded = loading.is_some_and(|loading| loading.is_column_loaded(column));
        let stale = wanted.changed == Changed::Yes
            || store.last_written(column) != wanted.written
            || (loaded && content.source != Source::Dimension);
        if stale {
            wanted.content = None;
            wanted.changed = Changed::No;
            wanted.ring_changed = Changed::No;
            wanted.written = store.last_written(column);
            wanted.light = Ask::NotAsked;
            return;
        }
        let Some(wanted) = self.wanted.remove(&column) else {
            return;
        };
        let Some(Content { mut stored, source }) = wanted.content else {
            return;
        };
        let relit = light.is_some();
        if let Some(light) = light {
            for section in light.sections {
                let Some(index) = section
                    .y
                    .checked_sub(stored.min_section_y)
                    .and_then(|index| usize::try_from(index).ok())
                else {
                    continue;
                };
                if let Some(held) = stored.sections.get_mut(index) {
                    held.block_light = Some(section.block_light);
                    held.sky_light = Some(section.sky_light);
                }
            }
        }
        let version = match source {
            Source::DataLayer(version) if relit => Some(version),
            Source::Generation => Some(ChunkVersion::default()),
            Source::DataLayer(_) | Source::Dimension => None,
        };
        if let Some(version) = version {
            store.put_lit(
                column,
                VersionedChunk {
                    version,
                    chunk: stored.clone(),
                },
            );
        }
        store.viewed.push(ViewedColumn {
            column,
            stored,
            delivery: wanted.delivery,
        });
    }

    pub(crate) fn note_block_changes(
        &mut self,
        changes: impl IntoIterator<Item = BlockChange>,
        lit_by_data_layer: impl Fn(ColumnPos) -> bool,
        store: &ColumnStoreLink,
        lights: Lights,
    ) {
        for BlockChange { pos, state } in changes {
            let column = ColumnPos::from(pos);
            if self.lit_world.contains(&column) {
                self.light_edits
                    .push(ColdLightEdit::SetBlock { pos, state });
            }
            if let Some(wanted) = self.wanted.get_mut(&column)
                && wanted
                    .content
                    .as_ref()
                    .is_some_and(|content| content.source == Source::Dimension)
            {
                wanted.changed = Changed::Yes;
            }
            if lights == Lights::No {
                continue;
            }
            for near in column.square(1) {
                match self.wanted.get_mut(&near) {
                    Some(wanted) if wanted.light == Ask::Asked => {
                        wanted.ring_changed = Changed::Yes;
                    }
                    Some(_) => {}
                    None => {
                        if lit_by_data_layer(near) {
                            self.relight(near, store);
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn take_store_answers(&mut self, store: &mut ColumnStoreLink, lights: Lights) {
        for fetched in store.take_fetched() {
            let column = fetched.column;
            if self
                .fetching
                .get(&column)
                .is_none_or(|source| *source == Source::Dimension)
            {
                continue;
            }
            match (fetched.answer, fetched.from) {
                (StoreOutput::Found { chunk, .. }, ReadFrom::Store) => {
                    self.arrive(
                        column,
                        chunk.chunk,
                        Source::DataLayer(chunk.version),
                        lights,
                    );
                }
                (StoreOutput::Found { chunk, .. }, ReadFrom::Generation) => {
                    self.arrive(column, chunk.chunk, Source::Generation, lights);
                }
                (StoreOutput::Missing { .. } | StoreOutput::Unreadable { .. }, _) => {
                    self.fetching.remove(&column);
                    self.wanted.remove(&column);
                }
            }
        }
    }

    pub(crate) fn take_snapshots(
        &mut self,
        dim_row: Option<(&ColumnLoading, &ColumnIndex, &DimensionTypeConfig)>,
        snapshots: &ColumnSnapshots,
        store: &mut ColumnStoreLink,
        lights: Lights,
        pending: &mut Vec<ColumnPos>,
    ) {
        pending.extend(
            self.fetching
                .iter()
                .filter(|(_, source)| **source == Source::Dimension)
                .map(|(column, _)| *column),
        );
        pending.sort_unstable();
        for column in pending.drain(..) {
            let Some((_, index, _)) =
                dim_row.filter(|(loading, ..)| loading.is_column_loaded(column))
            else {
                self.fetching.remove(&column);
                self.fetch(column, None, store);
                continue;
            };
            if let Some(stored) = index
                .get(&column)
                .and_then(|slot| snapshots.take_unlit(slot.entity))
            {
                self.arrive(column, stored, Source::Dimension, lights);
            }
        }
    }

    pub(crate) fn take_light(
        &mut self,
        loading: Option<&ColumnLoading>,
        store: &mut ColumnStoreLink,
    ) {
        for light in mem::take(&mut self.lit) {
            let column = light.column;
            let Some(wanted) = self
                .wanted
                .get_mut(&column)
                .filter(|wanted| wanted.light == Ask::Asked)
            else {
                continue;
            };
            if wanted.ring_changed == Changed::Yes {
                wanted.ring_changed = Changed::No;
                wanted.light = Ask::NotAsked;
                continue;
            }
            self.finish(column, Some(light), loading, store);
        }
    }

    pub(crate) fn move_wanted(
        &mut self,
        loading: Option<&ColumnLoading>,
        store: &mut ColumnStoreLink,
        lights: Lights,
        pending: &mut Vec<ColumnPos>,
    ) {
        pending.extend(self.wanted.keys().copied());
        pending.sort_unstable();
        for column in pending.drain(..) {
            let Some(wanted) = self.wanted.get(&column) else {
                continue;
            };
            let Some(content) = &wanted.content else {
                self.fetch(column, loading, store);
                continue;
            };
            if wanted.light != Ask::NotAsked {
                continue;
            }
            let final_light = content.stored.is_lit() && wanted.delivery == Delivery::Version;
            if lights == Lights::No || final_light {
                self.finish(column, None, loading, store);
                continue;
            }
            let mut ring_held = true;
            for neighbour in column.square(1) {
                if !self.lit_world.contains(&neighbour) {
                    ring_held = false;
                    self.fetch(neighbour, loading, store);
                }
            }
            if ring_held && let Some(wanted) = self.wanted.get_mut(&column) {
                wanted.light = Ask::Asked;
                self.light_awaits.push(column);
            }
        }
    }
}
