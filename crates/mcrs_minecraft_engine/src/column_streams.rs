use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Discard;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{
    Added, Changed, Component, Has, Local, On, Or, Query, Ref, Resource, With,
};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Res, ResMut};
use bevy_tasks::{ComputeTaskPool, ParallelSliceMut};
use bytes::Bytes;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::chunk_view::{
    ChunkTrackingView, PlayerViewDistance, SentChunkView,
};
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::{
    DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{ChunkSpawnSet, SectionEntitySet};
use mcrs_minecraft_engine_world::world::lifecycle::trace as column_trace;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, ColumnTraceLog};
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, ColumnLightSettled, LightGated, SectionRelit,
};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnLifecycleSet};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::warn;

use crate::column_change::ColumnChange;
use crate::column_packet::{DirectBits, StoredLight, encode_stored_column};
use crate::column_production::ColumnProduction;
use crate::column_request::ColumnRequest;
use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::{ColumnStoreLink, ViewedColumn};
use crate::column_version::ColumnVersion;
use crate::hot_ticket::HotColumns;
use crate::player_view::PlayerView;
use crate::stored_column::StoredColumn;

/// The version stream of each column of a dimension that the muxer subscribes to: the dimension
/// publishes a version of a column when the muxer wants one, and each change of a subscribed
/// column.
pub struct ColumnStreamPlugin;

/// Takes the requests of the muxer, reports the views of the players, and publishes the
/// columns that are ready.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnStreamSet;

impl Plugin for ColumnStreamPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            ColumnStreamSet
                .in_set(TickSet::Project)
                .after(ColumnLifecycleSet)
                .before(SectionEntitySet),
        );
        app.add_systems(
            FixedUpdate,
            (
                // The tickets must precede the spawn that follows in the same tick.
                (
                    release_published_tickets,
                    take_column_requests,
                    report_player_views,
                    land_viewed_columns,
                    requeue_unlit_columns,
                    raise_wanted_columns,
                )
                    .chain()
                    .before(ChunkSpawnSet),
                project_ready_columns
                    .after(raise_wanted_columns)
                    .before(ColumnLightGate),
                (promote_lit_columns, publish_ready_columns)
                    .chain()
                    .after(ColumnLightGate),
            )
                .in_set(ColumnStreamSet),
        );
        app.register_required_components::<ColumnLoading, ColumnStreams>();
        app.init_resource::<ColumnRequests>();
        app.add_message::<ColumnAwaitsLight>();
        app.add_message::<ColumnLightSettled>();
        app.add_message::<SectionRelit>();
        app.add_message::<PlayerView>();
        app.add_message::<ColumnVersion>();
        app.add_message::<ColumnChange>();
        app.add_observer(close_player_view);
    }
}

/// The requests of the muxer for the next tick, in the order it sent them.
#[derive(Resource, Default)]
pub struct ColumnRequests(pub Vec<ColumnRequest>);

/// Where a column waits before the dimension publishes a version of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    /// Wanted, with no work started for it yet.
    Queued,
    /// Waiting for the data layer to hand back the final column.
    Fetching,
    /// Waiting for its sections to land in the dimension.
    Awaiting,
    /// Landed and lit, waiting for the light around it to settle.
    Lighting,
    Ready,
}

impl Pending {
    /// Whether the stream holds a loading ticket on a column in this state. With a data layer
    /// the hot tickets load every column that the dimension publishes, so a stream holds none.
    fn holds_ticket(self, source: ViewSource) -> bool {
        source == ViewSource::Dimension
            && matches!(self, Self::Awaiting | Self::Lighting | Self::Ready)
    }
}

/// Where the dimension gets the columns that are not loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewSource {
    /// A wanted column is loaded with a ticket until the dimension publishes it.
    Dimension,
    /// The data layer makes each wanted column that the dimension does not light.
    DataLayer,
}

impl ViewSource {
    fn of(store: Option<&ColumnStoreLink>) -> Self {
        match store {
            Some(_) => Self::DataLayer,
            None => Self::Dimension,
        }
    }
}

/// The columns of a dimension that the muxer subscribes to, and the columns that it waits on
/// a version of.
#[derive(Component, Default)]
pub struct ColumnStreams {
    subscribed: FxHashSet<ColumnPos>,
    pending: FxHashMap<ColumnPos, Pending>,
    /// The loading tickets of the columns published in the last tick, to let go of.
    released_tickets: Vec<ColumnPos>,
}

impl ColumnStreams {
    /// Streams that publish the changes of `columns`, as if the muxer had asked for them and
    /// got their versions.
    pub fn subscribed_to(columns: impl IntoIterator<Item = ColumnPos>) -> Self {
        Self {
            subscribed: columns.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Whether the changes of `column` go to the muxer.
    #[must_use]
    pub fn is_subscribed(&self, column: ColumnPos) -> bool {
        self.subscribed.contains(&column)
    }

    fn state(&self, column: ColumnPos) -> Option<Pending> {
        self.pending.get(&column).copied()
    }

    /// The columns in `state`, in a fixed order, so the tickets and the requests that follow
    /// do not depend on the order of the map.
    fn in_state(&self, state: Pending, columns: &mut Vec<ColumnPos>) {
        columns.extend(
            self.pending
                .iter()
                .filter(|(_, pending)| **pending == state)
                .map(|(column, _)| *column),
        );
        columns.sort_unstable();
    }

    /// Moves `column` to `next`, and takes or lets go of its loading ticket to match.
    fn set(
        &mut self,
        loading: &mut ColumnLoading,
        source: ViewSource,
        column: ColumnPos,
        next: Option<Pending>,
    ) {
        let previous = match next {
            Some(next) => self.pending.insert(column, next),
            None => self.pending.remove(&column),
        };
        let held = previous.is_some_and(|previous| previous.holds_ticket(source));
        let holds = next.is_some_and(|next| next.holds_ticket(source));
        if held && !holds {
            loading.remove_ticket(column);
        } else if holds && !held {
            loading.add_ticket(column);
        }
    }

    /// Takes `column` out of the pending columns once its version is out. The send runs apart
    /// from the tickets, so its ticket goes in the next tick.
    fn published(&mut self, source: ViewSource, column: ColumnPos) {
        if self
            .pending
            .remove(&column)
            .is_some_and(|previous| previous.holds_ticket(source))
        {
            self.released_tickets.push(column);
        }
    }
}

fn release_published_tickets(mut dims: Query<(&mut ColumnLoading, &mut ColumnStreams)>) {
    for (mut loading, mut streams) in &mut dims {
        for column in streams.released_tickets.drain(..) {
            loading.remove_ticket(column);
        }
    }
}

/// Applies the requests of the muxer to the dimension of this world.
fn take_column_requests(
    mut requests: ResMut<ColumnRequests>,
    mut dims: Query<(&mut ColumnStreams, &mut ColumnLoading)>,
    store: Option<Res<ColumnStoreLink>>,
) {
    if requests.0.is_empty() {
        return;
    }
    let source = ViewSource::of(store.as_deref());
    // A dimension world holds one dimension.
    let Some((mut streams, mut loading)) = dims.iter_mut().next() else {
        requests.0.clear();
        return;
    };
    for request in requests.0.drain(..) {
        match request {
            ColumnRequest::Want(column) => {
                streams.subscribed.insert(column);
                if streams.state(column).is_none() {
                    streams.set(&mut loading, source, column, Some(Pending::Queued));
                }
            }
            ColumnRequest::Drop(column) => {
                streams.subscribed.remove(&column);
                streams.set(&mut loading, source, column, None);
            }
        }
    }
}

/// Reports where each player stands and how far it sees, once that changes.
pub(crate) fn report_player_views(
    mut players: Query<
        (
            &mut SentChunkView,
            &Transform,
            &PlayerViewDistance,
            &InDimension,
            &Owner,
        ),
        Or<(
            Changed<Transform>,
            Changed<PlayerViewDistance>,
            Added<SentChunkView>,
        )>,
    >,
    dims: Query<&DimensionTypeConfig>,
    mut views: MessageWriter<PlayerView>,
) {
    for (mut sent_view, transform, distance, in_dim, owner) in &mut players {
        let Ok(type_config) = dims.get(in_dim.0) else {
            continue;
        };
        let min_section_y = type_config.min_y >> SectionPos::BITS;
        let view = ChunkTrackingView::with_y_bounds(
            SectionPos::from(transform.translation),
            distance.distance + 1,
            distance.vert_distance + 1,
            min_section_y,
            min_section_y + type_config.section_count as i32 - 1,
        );
        if sent_view.0 == Some(view) {
            continue;
        }
        sent_view.0 = Some(view);
        views.write(PlayerView {
            session: owner.0,
            view: Some(view),
        });
    }
}

fn close_player_view(
    discard: On<Discard, SentChunkView>,
    owners: Query<&Owner>,
    mut views: MessageWriter<PlayerView>,
) {
    if let Ok(owner) = owners.get(discard.event().entity) {
        views.write(PlayerView {
            session: owner.0,
            view: None,
        });
    }
}

/// How the light of a version goes on the wire in this world.
fn stored_light(light_gated: bool, has_sky_light: bool) -> StoredLight {
    if light_gated {
        StoredLight::Kept { has_sky_light }
    } else {
        StoredLight::Fullbright
    }
}

/// Publishes each column that the data layer finished for the muxer. A column that the
/// dimension lights by now is taken from the dimension instead, so a version older than the
/// hot one never goes out.
pub(crate) fn land_viewed_columns(
    mut store: Option<ResMut<ColumnStoreLink>>,
    mut dims: Query<(
        &mut ColumnStreams,
        &mut ColumnLoading,
        &HotColumns,
        Has<HasSkyLight>,
    )>,
    light_gated: Option<Res<LightGated>>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut versions: MessageWriter<ColumnVersion>,
) {
    let Some(store) = store.as_deref_mut() else {
        return;
    };
    let direct_bits = DirectBits::of(&blocks, &registries);
    for ViewedColumn {
        dim,
        column,
        stored,
    } in store.take_viewed()
    {
        let Ok((mut streams, mut loading, hot, has_sky_light)) = dims.get_mut(dim) else {
            continue;
        };
        if streams.state(column) != Some(Pending::Fetching) {
            continue;
        }
        if hot.lights(column) {
            streams.set(
                &mut loading,
                ViewSource::DataLayer,
                column,
                Some(Pending::Queued),
            );
            continue;
        }
        streams.published(ViewSource::DataLayer, column);
        let light = stored_light(light_gated.is_some(), has_sky_light);
        // The encode logged why; the column stays unsent rather than produced again.
        if let Some(packet) = encode_stored_column(column, &stored, direct_bits, light) {
            versions.write(ColumnVersion { column, packet });
        }
    }
}

/// Sends a column that waits to be taken from the dimension back to the queue once the
/// dimension stops lighting it, so the data layer makes it instead.
fn requeue_unlit_columns(
    mut dims: Query<(&mut ColumnStreams, &mut ColumnLoading, Ref<HotColumns>)>,
    mut unlit: Local<Vec<ColumnPos>>,
) {
    for (mut streams, mut loading, hot) in &mut dims {
        if !hot.is_changed() {
            continue;
        }
        unlit.extend(
            streams
                .pending
                .iter()
                .filter(|(column, state)| {
                    matches!(state, Pending::Awaiting | Pending::Lighting) && !hot.lights(**column)
                })
                .map(|(column, _)| *column),
        );
        for column in unlit.drain(..) {
            streams.set(
                &mut loading,
                ViewSource::DataLayer,
                column,
                Some(Pending::Queued),
            );
        }
    }
}

/// Starts the work of each wanted column: the dimension waits for a column that it lights,
/// and the data layer makes the others. Without a data layer the column is loaded with a
/// ticket.
fn raise_wanted_columns(
    mut dims: Query<(
        Entity,
        &mut ColumnStreams,
        &mut ColumnLoading,
        Option<&HotColumns>,
    )>,
    store: Option<Res<ColumnStoreLink>>,
    mut production: Option<ResMut<ColumnProduction>>,
    traces: Option<Res<ColumnTraceLog>>,
    mut queued: Local<Vec<ColumnPos>>,
) {
    let source = ViewSource::of(store.as_deref());
    for (dim, mut streams, mut loading, hot) in &mut dims {
        streams.in_state(Pending::Queued, &mut queued);
        for column in queued.drain(..) {
            if let (Some(store), Some(production)) = (store.as_deref(), production.as_deref_mut()) {
                if hot.is_some_and(|hot| hot.lights(column)) {
                    streams.set(&mut loading, source, column, Some(Pending::Awaiting));
                } else {
                    streams.set(&mut loading, source, column, Some(Pending::Fetching));
                    production.want(dim, column, store);
                }
                continue;
            }
            streams.set(&mut loading, source, column, Some(Pending::Awaiting));
            column_trace::mark(traces.as_deref(), column, ColumnStage::Ticketed);
        }
    }
}

/// Whether the column entity and every section of `column` are in the dimension, each section
/// loaded. Light and heightmaps are read off the column entity, and a column published before
/// `reconcile_columns` indexed it would carry neither.
fn sections_landed(
    chunk_index: &SectionIndex,
    column_index: &ColumnIndex,
    stages: &Query<&SectionStage>,
    lit: Option<&Query<(), (With<BlockLight>, With<SkyLight>)>>,
    rows: &DimensionTypeConfig,
    column: ColumnPos,
) -> bool {
    if !column_index.0.contains_key(&column) {
        return false;
    }
    let first = rows.min_y >> SectionPos::BITS;
    (first..first + rows.section_count as i32).all(|y| {
        chunk_index
            .get(SectionPos::new(column.x, y, column.z))
            .is_some_and(|section| {
                stages.get(section) == Ok(&SectionStage::Loaded)
                    && lit.is_none_or(|lit| lit.contains(section))
            })
    })
}

/// Moves a column on from waiting once every section it carries has landed: to ready, or in
/// a light-gated world to waiting for the light around it to settle.
///
/// The light gate belongs before the publish: a client gets a column once, so a column whose
/// light has not settled stays black on the client.
fn project_ready_columns(
    mut dims: Query<(
        &SectionIndex,
        &ColumnIndex,
        &DimensionTypeConfig,
        &mut ColumnStreams,
    )>,
    stages: Query<&SectionStage>,
    lit: Query<(), (With<BlockLight>, With<SkyLight>)>,
    light_gated: Option<Res<LightGated>>,
    mut awaiting_light: MessageWriter<ColumnAwaitsLight>,
    mut awaiting: Local<Vec<ColumnPos>>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    let await_light = light_gated.is_some();
    for (chunk_index, column_index, rows, mut streams) in &mut dims {
        streams.in_state(Pending::Awaiting, &mut awaiting);
        for column in awaiting.drain(..) {
            // A cell's light is decided by the blocks within fifteen of it, which reaches one
            // column out and no further. So the neighbours owe this column their blocks, never
            // their light: waiting on their light too would hold every column of a bulk load
            // behind the whole queue.
            let own_light = await_light.then_some(&lit);
            let ready =
                sections_landed(chunk_index, column_index, &stages, own_light, rows, column)
                    && column.square(1).all(|neighbour| {
                        sections_landed(chunk_index, column_index, &stages, None, rows, neighbour)
                    });
            if !ready {
                continue;
            }
            let next = if await_light {
                awaiting_light.write(ColumnAwaitsLight(column));
                Pending::Lighting
            } else {
                column_trace::mark(traces.as_deref(), column, ColumnStage::Ready);
                Pending::Ready
            };
            streams.pending.insert(column, next);
        }
    }
}

/// Moves a column whose light has settled from waiting for it to ready.
fn promote_lit_columns(
    mut settled: MessageReader<ColumnLightSettled>,
    mut dims: Query<&mut ColumnStreams>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    for &ColumnLightSettled(column) in settled.read() {
        for mut streams in &mut dims {
            if streams.state(column) == Some(Pending::Lighting) {
                column_trace::mark(traces.as_deref(), column, ColumnStage::Ready);
                streams.pending.insert(column, Pending::Ready);
            }
        }
    }
}

/// A column taken from the sections of the dimension, and its packet once encoded.
struct ReadyColumn {
    column: ColumnPos,
    stored: StoredColumn,
    packet: Option<Bytes>,
}

/// Encodes every column of `ready`, spread over the compute pool when there is more than one.
fn encode_ready(mut ready: &mut [ReadyColumn], direct_bits: DirectBits, light: StoredLight) {
    let encode_all = |_, ready: &mut [ReadyColumn]| {
        for column in ready {
            column.packet = encode_stored_column(column.column, &column.stored, direct_bits, light);
        }
    };
    let pool = ComputeTaskPool::try_get();
    let threads = pool.map_or(1, |pool| pool.thread_num()).max(1);
    match pool.filter(|_| ready.len() > 1 && threads > 1) {
        Some(pool) => {
            // Columns differ in cost, so each thread gets several small chunks rather than one
            // share.
            let chunk = ready.len().div_ceil(threads * 4).max(1);
            ready.par_chunk_map_mut(pool, chunk, encode_all);
        }
        None => encode_all(0, ready),
    }
}

/// Publishes a version of each ready column, taken from the sections of the dimension and
/// encoded once for every client of the muxer that takes it.
#[expect(
    clippy::too_many_arguments,
    reason = "a system takes its params as arguments"
)]
fn publish_ready_columns(
    mut dims: Query<(&ColumnIndex, &mut ColumnStreams, Has<HasSkyLight>)>,
    snapshots: ColumnSnapshots,
    light_gated: Option<Res<LightGated>>,
    store: Option<Res<ColumnStoreLink>>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut versions: MessageWriter<ColumnVersion>,
    traces: Option<Res<ColumnTraceLog>>,
    mut ready: Local<Vec<ColumnPos>>,
    mut taken: Local<Vec<ReadyColumn>>,
) {
    let source = ViewSource::of(store.as_deref());
    let direct_bits = DirectBits::of(&blocks, &registries);
    for (column_index, mut streams, has_sky_light) in &mut dims {
        streams.in_state(Pending::Ready, &mut ready);
        for column in ready.drain(..) {
            let stored = column_index
                .0
                .get(&column)
                .and_then(|slot| snapshots.take(slot.entity));
            let Some(stored) = stored else {
                warn!(
                    col_x = column.x,
                    col_z = column.z,
                    "a ready column lost a section before it went out"
                );
                // Without tickets of its own, a stream waits for the column wherever it now is.
                // Neither move takes or lets go of a ticket.
                let next = match source {
                    ViewSource::Dimension => Pending::Awaiting,
                    ViewSource::DataLayer => Pending::Queued,
                };
                streams.pending.insert(column, next);
                continue;
            };
            column_trace::mark(traces.as_deref(), column, ColumnStage::Sent);
            streams.published(source, column);
            taken.push(ReadyColumn {
                column,
                stored,
                packet: None,
            });
        }
        encode_ready(
            &mut taken,
            direct_bits,
            stored_light(light_gated.is_some(), has_sky_light),
        );
        versions.write_batch(taken.drain(..).filter_map(|ready| {
            Some(ColumnVersion {
                column: ready.column,
                packet: ready.packet?,
            })
        }));
    }
}
