use std::mem;
use std::vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Local, Query, Res, ResMut, Resource};
use bevy_ecs::schedule::SystemSet;
use mcrs_engine_store::{ChunkVersion, StoreOutput, VersionedChunk};
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::entity::player::chunk_view::{ChunkTrackingView, SentChunkView};
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::light_gate::LightGated;
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::column_asks::ColumnOrigin;
use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::{ColumnStoreLink, Delivery, ViewedColumn};
use crate::column_streams::ColumnStreams;
use crate::foreign_views::ForeignViews;
use crate::hot_ticket::HotColumns;
use crate::stored_column::{LitBy, StoredColumn};

/// The stages in cold space: the data layer generates and lights each column that a view waits
/// on and that the dimension does not light, and hands the final column to the view.
///
/// A column is lit from its own blocks and the blocks of the eight around it, the reach of a
/// light level. The light stage of cold space holds the latest blocks of each column near a
/// view: a column takes them from the dimension while it is loaded there, else from the data
/// layer when the world wrote it, else from generation. A block change in a column that the
/// stage holds goes to the stage too. A column is final when its light and the light of the
/// eight around it have settled.
///
/// The game supplies the stages. Its generation takes [`Self::take_generation`] and answers
/// with [`Self::generated`]; its light takes [`Self::take_light_edits`] and
/// [`Self::take_light_awaits`] and answers with [`Self::lit`]. A world without a light stage
/// sends its columns unlit. The dimension that ticks never reads what this produces, so the
/// simulation does not depend on when a cold column is done.
#[derive(Resource, Default)]
pub struct ColumnProduction {
    wanted: FxHashMap<ColumnPos, Wanted>,
    /// The columns whose content is on its way, and where from.
    fetching: FxHashMap<ColumnPos, Fetch>,
    /// The columns whose blocks the light stage of cold space holds.
    lit_world: FxHashSet<ColumnPos>,
    generate: Vec<ColumnPos>,
    light_edits: Vec<ColdLightEdit>,
    light_awaits: Vec<ColumnPos>,
    generated: Vec<GeneratedColumn>,
    lit: Vec<ColumnLight>,
    /// The changes of the halo that the owners sent for this tick.
    halo_changes: Vec<BlockChange>,
    /// The views the columns were last screened against: the views of the players of the cell
    /// and of the other cells of the world.
    views: Vec<ChunkTrackingView>,
}

/// A block that changed, as the light stage of cold space takes it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockChange {
    pub(crate) pos: BlockPos,
    pub(crate) state: BlockStateId,
}

/// The columns that the views of the muxer hold from a dimension.
#[derive(Clone, Copy)]
struct HeldColumns<'a> {
    dim: Entity,
    streams: &'a ColumnStreams,
    hot: Option<&'a HotColumns>,
}

impl HeldColumns<'_> {
    /// Whether views hold `column` with the light of the data layer: the muxer subscribes to
    /// it and waits on no version of it, and the dimension does not light it.
    fn lit_by_data_layer(&self, column: ColumnPos) -> bool {
        self.streams.is_subscribed(column)
            && !self.streams.is_pending(column)
            && self.hot.is_none_or(|hot| !hot.lights(column))
    }
}

/// Where the stages of cold space hand their answers to the data layer, at the start of the
/// tick, after the answers of the data layer itself.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColdStageLandSet {
    Generation,
    Light,
}

/// An edit for the light stage of cold space.
#[derive(Clone, Debug)]
pub enum ColdLightEdit {
    Load {
        pos: SectionPos,
        blocks: ChunkBlocks,
    },
    Unload {
        pos: SectionPos,
    },
    SetBlock {
        pos: BlockPos,
        state: BlockStateId,
    },
}

/// The settled light of one column from the light stage of cold space.
#[derive(Clone, Debug)]
pub struct ColumnLight {
    pub column: ColumnPos,
    pub sections: Vec<LitSection>,
}

/// The light of one section of a [`ColumnLight`].
#[derive(Clone, Debug)]
pub struct LitSection {
    pub y: i32,
    pub block_light: BlockLight,
    pub sky_light: SkyLight,
}

struct GeneratedColumn {
    column: ColumnPos,
    stored: Option<StoredColumn>,
}

/// A column that a view waits on, or a column that views hold and whose light is lit again.
struct Wanted {
    dim: Entity,
    delivery: Delivery,
    content: Option<Content>,
    light: Ask,
    /// The last version the world wrote when the content was fetched. A later write makes
    /// the content stale.
    written: Option<ChunkVersion>,
    /// A block of the column changed since its content was taken from the dimension.
    changed: Changed,
    /// A block within the reach of light changed after the light was asked for, so the answer
    /// may lack it.
    ring_changed: Changed,
}

struct Content {
    stored: StoredColumn,
    source: Source,
}

/// Where the content of a column came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Dimension,
    DataLayer(ChunkVersion),
    Generation,
}

#[derive(Clone, Copy, Debug)]
struct Fetch {
    dim: Entity,
    source: Source,
}

/// Whether the light stage owes a wanted column its light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    NotAsked,
    Asked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Changed {
    No,
    Yes,
}

/// Whether the dimension world lights its columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lights {
    Yes,
    No,
}

impl Wanted {
    fn new(dim: Entity, delivery: Delivery, written: Option<ChunkVersion>) -> Self {
        Self {
            dim,
            delivery,
            content: None,
            light: Ask::NotAsked,
            written,
            changed: Changed::No,
            ring_changed: Changed::No,
        }
    }
}

impl ColumnProduction {
    /// The columns to generate whole, for the generation stage of cold space.
    pub fn take_generation(&mut self) -> vec::Drain<'_, ColumnPos> {
        self.generate.drain(..)
    }

    /// The answer of generation for `column`: its blocks, block entities and heightmaps, or
    /// `None` when generation dropped it.
    pub fn generated(&mut self, column: ColumnPos, stored: Option<StoredColumn>) {
        self.generated.push(GeneratedColumn { column, stored });
    }

    /// The edits for the light stage of cold space, in order.
    pub fn take_light_edits(&mut self) -> vec::Drain<'_, ColdLightEdit> {
        self.light_edits.drain(..)
    }

    /// The columns to answer with their light once it settles, after the edits of the same
    /// tick.
    pub fn take_light_awaits(&mut self) -> vec::Drain<'_, ColumnPos> {
        self.light_awaits.drain(..)
    }

    /// The settled light of a column that [`Self::take_light_awaits`] asked for.
    pub fn lit(&mut self, light: ColumnLight) {
        self.lit.push(light);
    }

    /// The views that the stages of cold space serve, for a stage that drops what left them.
    #[must_use]
    pub fn views(&self) -> &[ChunkTrackingView] {
        &self.views
    }

    /// Whether no column waits on a stage of cold space, and the light stage holds no column.
    pub(crate) fn is_idle(&self) -> bool {
        self.wanted.is_empty()
            && self.fetching.is_empty()
            && self.lit_world.is_empty()
            && self.generate.is_empty()
            && self.light_edits.is_empty()
            && self.light_awaits.is_empty()
            && self.generated.is_empty()
            && self.lit.is_empty()
            && self.halo_changes.is_empty()
    }

    pub(crate) fn note_halo_changes(&mut self, changes: impl IntoIterator<Item = BlockChange>) {
        self.halo_changes.extend(changes);
    }

    /// Produces `column` for the views of `dim` that wait on it, once for all of them.
    pub(crate) fn want(&mut self, dim: Entity, column: ColumnPos, store: &ColumnStoreLink) {
        self.wanted
            .entry(column)
            .or_insert_with(|| Wanted::new(dim, Delivery::Version, store.last_written(column)))
            .delivery = Delivery::Version;
    }

    /// Lights `column`, which views hold, again and hands its light to them.
    fn relight(&mut self, dim: Entity, column: ColumnPos, store: &ColumnStoreLink) {
        self.wanted
            .entry(column)
            .or_insert_with(|| Wanted::new(dim, Delivery::Light, store.last_written(column)));
    }

    /// Sends `column` on its way from the dimension, the data layer or generation.
    fn fetch(
        &mut self,
        dim: Entity,
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
            store.fetch(dim, column);
            Source::DataLayer(ChunkVersion::default())
        } else {
            self.generate.push(column);
            Source::Generation
        };
        self.fetching.insert(column, Fetch { dim, source });
    }

    /// Sends `column` to generation after the data layer held no version of it.
    fn fetch_generated(&mut self, dim: Entity, column: ColumnPos) {
        self.generate.push(column);
        self.fetching.insert(
            column,
            Fetch {
                dim,
                source: Source::Generation,
            },
        );
    }

    /// Takes in the content of `column`: into the light stage, and into the wanted column.
    fn arrive(&mut self, column: ColumnPos, stored: StoredColumn, source: Source, lights: Lights) {
        self.fetching.remove(&column);
        if lights == Lights::Yes && within(&self.views, column, 1) {
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

    /// Drops what left every view: the wanted columns, and the columns of the light stage that
    /// no wanted column can need.
    fn screen(&mut self, views: Vec<ChunkTrackingView>, rows: Option<&DimensionTypeConfig>) {
        if views == self.views {
            return;
        }
        self.views = views;
        let views = &self.views;
        self.wanted.retain(|column, _| within(views, *column, 0));
        let Some(rows) = rows else {
            return;
        };
        let first = rows.min_y >> SectionPos::BITS;
        let count = i32::try_from(rows.section_count).unwrap_or(0);
        let light_edits = &mut self.light_edits;
        self.lit_world.retain(|column| {
            if within(views, *column, 1) {
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

    /// Hands a wanted column whose light is in to its views, or fetches it again when its
    /// content went stale meanwhile.
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
            stored.lit_by = LitBy::DataLayer;
        }
        let version = match source {
            Source::DataLayer(version) if relit => Some(version),
            Source::Generation => Some(ChunkVersion::default()),
            Source::DataLayer(_) | Source::Dimension => None,
        };
        // The link keeps a column that the disk did not read from this write.
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
            dim: wanted.dim,
            column,
            stored,
            delivery: wanted.delivery,
        });
    }
}

impl ColumnProduction {
    /// Hands each block change to the light stage of cold space, and marks a wanted column
    /// whose content the dimension gave as changed. A light level reaches one column out, so a
    /// column within one column of the change that views hold with light of the data layer is
    /// lit again, and an answer of light asked for before the change is asked for again.
    fn note_block_changes(
        &mut self,
        changes: impl IntoIterator<Item = BlockChange>,
        held: Option<HeldColumns>,
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
                        if let Some(held) = held
                            && held.lit_by_data_layer(near)
                        {
                            self.relight(held.dim, near, store);
                        }
                    }
                }
            }
        }
    }

    /// Takes the answers of the data layer: a column it holds arrives, a column it does not
    /// hold goes to generation.
    fn take_store_answers(&mut self, store: &mut ColumnStoreLink, lights: Lights) {
        for fetched in store.take_fetched() {
            let column = fetched.column;
            let Some(fetch) = self.fetching.get(&column).copied() else {
                continue;
            };
            if !matches!(fetch.source, Source::DataLayer(_)) {
                continue;
            }
            match fetched.answer {
                StoreOutput::Found { chunk, .. } => {
                    self.arrive(
                        column,
                        chunk.chunk,
                        Source::DataLayer(chunk.version),
                        lights,
                    );
                }
                StoreOutput::Missing { .. } | StoreOutput::Unreadable { .. } => {
                    self.fetch_generated(fetch.dim, column);
                }
            }
        }
    }

    fn take_generated(&mut self, lights: Lights) {
        for GeneratedColumn { column, stored } in mem::take(&mut self.generated) {
            if self.fetching.get(&column).map(|fetch| fetch.source) != Some(Source::Generation) {
                continue;
            }
            match stored {
                Some(stored) => self.arrive(column, stored, Source::Generation, lights),
                None => {
                    self.fetching.remove(&column);
                    self.wanted.remove(&column);
                }
            }
        }
    }

    /// Takes the content of each column fetched from the dimension once all its sections
    /// landed, or fetches it elsewhere once the dimension stops loading it.
    fn take_snapshots(
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
                .filter(|(_, fetch)| fetch.source == Source::Dimension)
                .map(|(column, _)| *column),
        );
        pending.sort_unstable();
        for column in pending.drain(..) {
            let Some(fetch) = self.fetching.get(&column).copied() else {
                continue;
            };
            let Some((_, index, _)) =
                dim_row.filter(|(loading, ..)| loading.is_column_loaded(column))
            else {
                self.fetching.remove(&column);
                self.fetch(fetch.dim, column, None, store);
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

    fn take_light(&mut self, loading: Option<&ColumnLoading>, store: &mut ColumnStoreLink) {
        for light in mem::take(&mut self.lit) {
            let column = light.column;
            let asked = self
                .wanted
                .get(&column)
                .is_some_and(|wanted| wanted.light == Ask::Asked);
            if !asked {
                continue;
            }
            if let Some(wanted) = self.wanted.get_mut(&column)
                && wanted.ring_changed == Changed::Yes
            {
                wanted.ring_changed = Changed::No;
                wanted.light = Ask::NotAsked;
                continue;
            }
            self.finish(column, Some(light), loading, store);
        }
    }

    /// Fetches what each wanted column lacks, and asks for its light once the eight around it
    /// are in the light stage.
    fn move_wanted(
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
            let dim = wanted.dim;
            let Some(content) = &wanted.content else {
                self.fetch(dim, column, loading, store);
                continue;
            };
            if wanted.light != Ask::NotAsked {
                continue;
            }
            let final_light =
                content.stored.lit_by == LitBy::DataLayer && wanted.delivery == Delivery::Version;
            if lights == Lights::No || final_light {
                self.finish(column, None, loading, store);
                continue;
            }
            let mut ring_held = true;
            for neighbour in column.square(1) {
                if !self.lit_world.contains(&neighbour) {
                    ring_held = false;
                    self.fetch(dim, neighbour, loading, store);
                }
            }
            if ring_held && let Some(wanted) = self.wanted.get_mut(&column) {
                wanted.light = Ask::Asked;
                self.light_awaits.push(column);
            }
        }
    }
}

/// Moves each column that a view waits on along: fetches its content and the blocks of the
/// eight around it, asks for its light, and hands the final column to the views.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn advance_production(
    mut production: ResMut<ColumnProduction>,
    mut store: ResMut<ColumnStoreLink>,
    dims: Query<(&ColumnLoading, &ColumnIndex, &DimensionTypeConfig)>,
    held: Query<(Entity, &ColumnStreams, Option<&HotColumns>)>,
    snapshots: ColumnSnapshots,
    sent_views: Query<&SentChunkView>,
    foreign_views: Option<Res<ForeignViews>>,
    mut placed: MessageReader<BlockPlaced>,
    light_gated: Option<Res<LightGated>>,
    mut pending: Local<Vec<ColumnPos>>,
) {
    let lights = match light_gated {
        Some(_) => Lights::Yes,
        None => Lights::No,
    };
    let store = &mut *store;
    // A dimension world holds one dimension.
    let dim_row = dims.iter().next();
    let loading = dim_row.map(|(loading, ..)| loading);
    let mut views: Vec<ChunkTrackingView> = sent_views.iter().filter_map(|view| view.0).collect();
    views.extend(foreign_views.iter().flat_map(|foreign| foreign.views()));
    production.screen(views, dim_row.map(|(.., rows)| rows));
    let held = held
        .iter()
        .next()
        .map(|(dim, streams, hot)| HeldColumns { dim, streams, hot });
    // A halo change that the dimension applied comes again as a placed block, and a block set
    // to the state it holds changes nothing.
    let halo_changes = mem::take(&mut production.halo_changes);
    let changes = placed
        .read()
        .map(|placed| BlockChange {
            pos: placed.block_pos,
            state: placed.new_state,
        })
        .chain(halo_changes);
    production.note_block_changes(changes, held, store, lights);
    production.take_store_answers(store, lights);
    production.take_generated(lights);
    production.take_snapshots(dim_row, &snapshots, store, lights, &mut pending);
    production.take_light(loading, store);
    production.move_wanted(loading, store, lights, &mut pending);
}

/// Whether some view reaches `column` with `slack` columns to spare.
fn within(views: &[ChunkTrackingView], column: ColumnPos, slack: i32) -> bool {
    views.iter().any(|view| {
        let reach = i64::from(view.distance) + i64::from(slack);
        i64::from(column.x.abs_diff(view.center.x)) <= reach
            && i64::from(column.z.abs_diff(view.center.z)) <= reach
    })
}
