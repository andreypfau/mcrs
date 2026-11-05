use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::lifecycle::Discard;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{
    Added, Changed, Component, Has, Local, On, Or, Query, Ref, Resource, Single, With, Without,
};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Res, ResMut};
use bevy_tasks::{ComputeTaskPool, ParallelSliceMut};
use bytes::Bytes;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::chunk_view::{
    ChunkTrackingView, PlayerViewDistance, SentChunkView, VIEW_MARGIN,
};
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::{
    DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::{SectionStage, SectionStageChanged};
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{ChunkSpawnSet, SectionEntitySet};
use mcrs_minecraft_engine_world::world::lifecycle::trace as column_trace;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, ColumnTraceLog};
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, ColumnLightSettled, LightGated, SectionRelit,
};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnLifecycleSet};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::chunk::LightData;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLightUpdate;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::warn;

use crate::bus::{PacketPayload, PacketPriority};
use crate::column_change::ColumnChange;
use crate::column_packet::{DirectBits, StoredLight, encode_stored_column};
use crate::column_production::ColumnProduction;
use crate::column_request::ColumnRequest;
use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::{ColumnStoreLink, Delivery, ViewedColumn};
use crate::column_version::ColumnVersion;
use crate::held_light::{HeldLight, RelitSections};
use crate::hot_ticket::CellCrossing;
use crate::light_codec::{LightCodecParams, build_delta_light_data, build_stored_light_data};
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
                (
                    take_column_requests,
                    report_player_views,
                    land_viewed_columns,
                    requeue_unlit_columns,
                    raise_wanted_columns,
                )
                    .chain()
                    .before(ChunkSpawnSet),
                (note_relit_columns, project_ready_columns)
                    .chain()
                    .after(raise_wanted_columns)
                    .before(ColumnLightGate),
                (take_lit_columns, publish_ready_columns)
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
    /// Waiting for its sections and the sections around it to land in the dimension.
    Awaiting,
    /// Landed and lit, waiting for the light around it to be final.
    Lighting,
    Ready,
}

/// The columns of a dimension that the muxer subscribes to, the columns that it waits on a
/// version of, and the light that the clients hold of the others.
#[derive(Component, Default)]
pub struct ColumnStreams {
    subscribed: FxHashSet<ColumnPos>,
    pending: FxHashMap<ColumnPos, Pending>,
    /// The light that went out with each column, and with each change of its light since.
    held: FxHashMap<ColumnPos, HeldLight>,
    /// The sections of each held column that the dimension lit again since the light went out,
    /// until the light of the column is final.
    relit: FxHashMap<ColumnPos, RelitSections>,
    /// The columns that began to wait for their sections this tick.
    awaited: Vec<ColumnPos>,
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

    /// Whether the clients hold a version of `column`: the muxer subscribes to it and waits on
    /// no version of it.
    pub(crate) fn holds(&self, column: ColumnPos) -> bool {
        self.is_subscribed(column) && !self.pending.contains_key(&column)
    }

    fn state(&self, column: ColumnPos) -> Option<Pending> {
        self.pending.get(&column).copied()
    }

    /// The columns in `state`, in a fixed order, so the requests that follow do not depend on
    /// the order of the map.
    fn in_state(&self, state: Pending, columns: &mut Vec<ColumnPos>) {
        columns.extend(
            self.pending
                .iter()
                .filter(|(_, pending)| **pending == state)
                .map(|(column, _)| *column),
        );
        columns.sort_unstable();
    }

    fn set(&mut self, column: ColumnPos, next: Pending) {
        self.pending.insert(column, next);
    }

    fn forget(&mut self, column: ColumnPos) {
        self.subscribed.remove(&column);
        self.pending.remove(&column);
        self.held.remove(&column);
        self.relit.remove(&column);
    }

    /// Takes `column` out of the pending columns as `stored` goes out, with its light.
    fn sent(&mut self, column: ColumnPos, stored: &StoredColumn) {
        self.pending.remove(&column);
        self.relit.remove(&column);
        self.held.insert(column, HeldLight::of(stored));
    }
}

/// Applies the requests of the muxer to the dimension of this world.
fn take_column_requests(
    mut requests: ResMut<ColumnRequests>,
    streams: Option<Single<&mut ColumnStreams>>,
) {
    let Some(mut streams) = streams else {
        requests.0.clear();
        return;
    };
    for request in requests.0.drain(..) {
        match request {
            ColumnRequest::Want(column) => {
                streams.subscribed.insert(column);
                if streams.state(column).is_none() {
                    streams.set(column, Pending::Queued);
                }
            }
            ColumnRequest::Drop(column) => streams.forget(column),
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
            distance.distance.saturating_add(VIEW_MARGIN),
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
    owners: Query<&Owner, Without<CellCrossing>>,
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

fn light_change(column: ColumnPos, light_data: LightData<'static>) -> ColumnChange {
    ColumnChange {
        column,
        // A column's light is sent in full exactly once, so a shed delta is never re-sent;
        // Normal is the class `dispatch_encode` drops first.
        priority: PacketPriority::High,
        payload: PacketPayload::LightUpdate(ClientboundLightUpdate {
            x: VarInt(column.x),
            z: VarInt(column.z),
            light_data,
        }),
    }
}

/// Publishes each column that the data layer finished for the muxer. A column that the
/// dimension lights by now is taken from the dimension instead, so a version older than the
/// hot one never goes out. A column that the views hold and that the data layer lit again goes
/// out as a change of its light, unless the clients hold that light.
#[expect(
    clippy::too_many_arguments,
    reason = "a system takes its params as arguments"
)]
pub(crate) fn land_viewed_columns(
    mut store: ResMut<ColumnStoreLink>,
    dims: Single<(&mut ColumnStreams, &ColumnLoading, Has<HasSkyLight>)>,
    light_gated: Option<Res<LightGated>>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut versions: MessageWriter<ColumnVersion>,
    mut changes: MessageWriter<ColumnChange>,
    mut taken: Local<Vec<ReadyColumn>>,
) {
    let direct_bits = DirectBits::of(&blocks, &registries);
    let (mut streams, loading, has_sky_light) = dims.into_inner();
    for ViewedColumn {
        column,
        stored,
        delivery,
    } in store.take_viewed()
    {
        match delivery {
            Delivery::Light => {
                // The dimension sends the light of a column that it lights.
                if light_gated.is_none() || !streams.holds(column) || loading.lights(column) {
                    continue;
                }
                let next = HeldLight::of(&stored);
                if streams.held.get(&column) == Some(&next) {
                    continue;
                }
                streams.held.insert(column, next);
                let light_data = build_stored_light_data(&stored, has_sky_light);
                changes.write(light_change(column, light_data));
            }
            Delivery::Version => {
                if streams.state(column) != Some(Pending::Fetching) {
                    continue;
                }
                if loading.lights(column) {
                    streams.set(column, Pending::Queued);
                    continue;
                }
                streams.sent(column, &stored);
                taken.push(ReadyColumn {
                    column,
                    stored,
                    light: stored_light(light_gated.is_some(), has_sky_light),
                    packet: None,
                });
            }
        }
    }
    encode_ready(&mut taken, direct_bits, &mut versions);
}

/// Sends a column that waits to be taken from the dimension back to the queue once the
/// dimension stops lighting it, so the data layer makes it instead.
fn requeue_unlit_columns(
    mut dims: Query<(&mut ColumnStreams, Ref<ColumnLoading>)>,
    mut unlit: Local<Vec<ColumnPos>>,
) {
    for (mut streams, loading) in &mut dims {
        if !loading.is_changed() {
            continue;
        }
        unlit.extend(
            streams
                .pending
                .iter()
                .filter(|(column, state)| {
                    matches!(state, Pending::Awaiting | Pending::Lighting)
                        && !loading.lights(**column)
                })
                .map(|(column, _)| *column),
        );
        for column in unlit.drain(..) {
            streams.set(column, Pending::Queued);
        }
    }
}

/// Starts the work of each wanted column: the dimension waits for a column that it lights,
/// and the data layer makes the others.
fn raise_wanted_columns(
    mut dims: Query<(&mut ColumnStreams, &ColumnLoading)>,
    store: Res<ColumnStoreLink>,
    mut production: ResMut<ColumnProduction>,
    mut queued: Local<Vec<ColumnPos>>,
) {
    for (mut streams, loading) in &mut dims {
        streams.in_state(Pending::Queued, &mut queued);
        for column in queued.drain(..) {
            if loading.lights(column) {
                streams.set(column, Pending::Awaiting);
                streams.awaited.push(column);
            } else {
                streams.set(column, Pending::Fetching);
                production.want(column, &store);
            }
        }
    }
}

/// Keeps the sections of each held column that the dimension lit again, and asks the light
/// stage to answer once the light of the column is final.
fn note_relit_columns(
    mut relit: MessageReader<SectionRelit>,
    positions: Query<&SectionPos>,
    streams: Option<Single<&mut ColumnStreams>>,
    mut awaiting_light: MessageWriter<ColumnAwaitsLight>,
    mut columns: Local<Vec<ColumnPos>>,
) {
    let Some(mut streams) = streams else {
        return;
    };
    for change in relit.read() {
        let Ok(pos) = positions.get(change.section) else {
            continue;
        };
        let column = ColumnPos::from(*pos);
        if !streams.holds(column) {
            continue;
        }
        streams.relit.entry(column).or_default().note(change);
        columns.push(column);
    }
    columns.sort_unstable();
    columns.dedup();
    awaiting_light.write_batch(columns.drain(..).map(ColumnAwaitsLight));
}

/// Whether the column entity and every section of `column` are in the dimension, each section
/// loaded. Light and heightmaps are read off the column entity, and a column published before
/// `reconcile_columns` indexed it would carry neither.
fn sections_landed(
    chunk_index: &SectionIndex,
    stages: &Query<&SectionStage>,
    lit: Option<&Query<(), (With<BlockLight>, With<SkyLight>)>>,
    rows: &DimensionTypeConfig,
    column: ColumnPos,
) -> bool {
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
/// a light-gated world to waiting for the light around it to be final. A column is looked at
/// when it begins to wait, when a section lands within one column of it, and when its own light
/// is written.
///
/// The light gate belongs before the publish: a client gets a column once, so a column whose
/// light is not final stays black on the client.
#[expect(
    clippy::too_many_arguments,
    reason = "a system takes its params as arguments"
)]
fn project_ready_columns(
    dims: Option<
        Single<(
            &SectionIndex,
            &ColumnIndex,
            &DimensionTypeConfig,
            &mut ColumnStreams,
        )>,
    >,
    mut changes: MessageReader<SectionStageChanged>,
    mut relit: MessageReader<SectionRelit>,
    positions: Query<&SectionPos>,
    stages: Query<&SectionStage>,
    lit: Query<(), (With<BlockLight>, With<SkyLight>)>,
    light_gated: Option<Res<LightGated>>,
    mut awaiting_light: MessageWriter<ColumnAwaitsLight>,
    mut candidates: Local<Vec<ColumnPos>>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    let Some(dims) = dims else {
        return;
    };
    let (chunk_index, column_index, rows, mut streams) = dims.into_inner();
    candidates.append(&mut streams.awaited);
    for change in changes.read().filter(|change| change.landed()) {
        candidates.extend(ColumnPos::from(change.pos).square(1));
    }
    for change in relit.read() {
        if let Ok(pos) = positions.get(change.section) {
            candidates.push(ColumnPos::from(*pos));
        }
    }
    if streams.pending.is_empty() {
        candidates.clear();
        return;
    }
    candidates.sort_unstable();
    candidates.dedup();
    let await_light = light_gated.is_some();
    for column in candidates.drain(..) {
        if streams.state(column) != Some(Pending::Awaiting) || !column_index.0.contains_key(&column)
        {
            continue;
        }
        // A cell's light is decided by the blocks within fifteen of it, which reaches one
        // column out and no further. So the neighbours owe this column their blocks, never
        // their light: waiting on their light too would hold every column of a bulk load
        // behind the whole queue.
        let ready = column.square(1).all(|near| {
            let own_light = (await_light && near == column).then_some(&lit);
            sections_landed(chunk_index, &stages, own_light, rows, near)
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
        streams.set(column, next);
    }
}

/// Moves a column whose light is final from waiting for it to ready, and sends the change of
/// the light of a held column that the clients do not hold.
fn take_lit_columns(
    mut settled: MessageReader<ColumnLightSettled>,
    dims: Option<Single<(&ColumnIndex, &mut ColumnStreams)>>,
    light: Query<(&SectionPos, &BlockLight, &SkyLight)>,
    codec_params: LightCodecParams,
    mut changes: MessageWriter<ColumnChange>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    let Some(dims) = dims else {
        return;
    };
    let (column_index, mut streams) = dims.into_inner();
    for &ColumnLightSettled(column) in settled.read() {
        if streams.state(column) == Some(Pending::Lighting) {
            column_trace::mark(traces.as_deref(), column, ColumnStage::Ready);
            streams.set(column, Pending::Ready);
            continue;
        }
        let Some(mut relit) = streams.relit.remove(&column) else {
            continue;
        };
        let Some(slot) = column_index.get(&column) else {
            continue;
        };
        relit.retain_unheld(streams.held.get_mut(&column), &light);
        if relit.is_empty() {
            continue;
        }
        let light_data =
            build_delta_light_data(slot.entity, &relit.block, &relit.sky, &codec_params);
        changes.write(light_change(column, light_data));
    }
}

/// A column taken from the sections of the dimension, and its packet once encoded.
pub(crate) struct ReadyColumn {
    column: ColumnPos,
    stored: StoredColumn,
    light: StoredLight,
    packet: Option<Bytes>,
}

/// Encodes every column of `ready`, spread over the compute pool when there is more than one,
/// and publishes a version of each one it took off `ready`.
fn encode_ready(
    ready: &mut Vec<ReadyColumn>,
    direct_bits: DirectBits,
    versions: &mut MessageWriter<ColumnVersion>,
) {
    let encode_all = |_, ready: &mut [ReadyColumn]| {
        for column in ready {
            column.packet =
                encode_stored_column(column.column, &column.stored, direct_bits, column.light);
        }
    };
    let pool = ComputeTaskPool::try_get();
    let threads = pool.map_or(1, |pool| pool.thread_num()).max(1);
    let mut columns = ready.as_mut_slice();
    match pool.filter(|_| columns.len() > 1 && threads > 1) {
        Some(pool) => {
            // Columns differ in cost, so each thread gets several small chunks rather than one
            // share.
            let chunk = columns.len().div_ceil(threads * 4).max(1);
            columns.par_chunk_map_mut(pool, chunk, encode_all);
        }
        None => encode_all(0, columns),
    }
    // The encode logged why a column has no packet; it stays unsent rather than produced again.
    versions.write_batch(ready.drain(..).filter_map(|ready| {
        Some(ColumnVersion {
            column: ready.column,
            packet: ready.packet?,
        })
    }));
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
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut versions: MessageWriter<ColumnVersion>,
    traces: Option<Res<ColumnTraceLog>>,
    mut ready: Local<Vec<ColumnPos>>,
    mut taken: Local<Vec<ReadyColumn>>,
) {
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
                streams.set(column, Pending::Queued);
                continue;
            };
            column_trace::mark(traces.as_deref(), column, ColumnStage::Sent);
            streams.sent(column, &stored);
            taken.push(ReadyColumn {
                column,
                stored,
                light: stored_light(light_gated.is_some(), has_sky_light),
                packet: None,
            });
        }
        encode_ready(&mut taken, direct_bits, &mut versions);
    }
}
