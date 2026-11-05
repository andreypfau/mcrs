use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Discard;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{
    Added, Changed, Commands, Component, Has, Local, On, Or, Query, Ref, Resource, Single, With,
    Without,
};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Res, ResMut, SystemParam};
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
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{ChunkSpawnSet, SectionEntitySet};
use mcrs_minecraft_engine_world::world::lifecycle::trace as column_trace;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, ColumnTraceLog};
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, ColumnLightSettled, LightGated, SectionRelit, SentLight,
};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnLifecycleSet};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_protocol::VarInt;
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
use crate::hot_ticket::{CellCrossing, HotColumns};
use crate::light_codec::build_stored_light_data;
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
                    clear_lit_this_tick,
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

/// The columns of a dimension that the muxer subscribes to, and the columns that it waits on
/// a version of.
#[derive(Component, Default)]
pub struct ColumnStreams {
    subscribed: FxHashSet<ColumnPos>,
    pending: FxHashMap<ColumnPos, Pending>,
    /// The columns whose version this tick took from the sections of the dimension, after the
    /// light of the tick was written into them.
    lit_this_tick: FxHashSet<ColumnPos>,
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

    /// Whether this tick published a version of `column` that holds the light that the tick
    /// wrote, so that light is no change to the muxer.
    #[must_use]
    pub fn published_light_of_this_tick(&self, column: ColumnPos) -> bool {
        self.lit_this_tick.contains(&column)
    }

    /// Whether the muxer waits on a version of `column`.
    pub(crate) fn is_pending(&self, column: ColumnPos) -> bool {
        self.pending.contains_key(&column)
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

    fn set(&mut self, column: ColumnPos, next: Pending) {
        self.pending.insert(column, next);
    }
}

fn clear_lit_this_tick(mut dims: Query<&mut ColumnStreams>) {
    for mut streams in &mut dims {
        streams.lit_this_tick.clear();
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
            ColumnRequest::Drop(column) => {
                streams.subscribed.remove(&column);
                streams.pending.remove(&column);
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

/// Publishes each column that the data layer finished for the muxer. A column that the
/// dimension lights by now is taken from the dimension instead, so a version older than the
/// hot one never goes out. A column that the views hold and that the data layer lit again goes
/// out as a change of its light.
#[expect(
    clippy::too_many_arguments,
    reason = "a system takes its params as arguments"
)]
pub(crate) fn land_viewed_columns(
    mut store: ResMut<ColumnStoreLink>,
    mut dims: Query<(
        &mut ColumnStreams,
        &HotColumns,
        &SectionIndex,
        Has<HasSkyLight>,
    )>,
    light_gated: Option<Res<LightGated>>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut versions: MessageWriter<ColumnVersion>,
    mut changes: MessageWriter<ColumnChange>,
    mut sent_lights: SentLights,
    mut taken: Local<Vec<ReadyColumn>>,
) {
    let direct_bits = DirectBits::of(&blocks, &registries);
    for ViewedColumn {
        dim,
        column,
        stored,
        delivery,
    } in store.take_viewed()
    {
        let Ok((mut streams, hot, sections, has_sky_light)) = dims.get_mut(dim) else {
            continue;
        };
        if delivery == Delivery::Light {
            let held = streams.is_subscribed(column)
                && streams.state(column).is_none()
                && !hot.lights(column);
            if held && light_gated.is_some() {
                sent_lights.note(sections, column, &stored, Unkept::Keep);
                changes.write(ColumnChange {
                    column,
                    priority: PacketPriority::High,
                    payload: PacketPayload::LightUpdate(ClientboundLightUpdate {
                        x: VarInt(column.x),
                        z: VarInt(column.z),
                        light_data: build_stored_light_data(&stored, has_sky_light),
                    }),
                });
            }
            continue;
        }
        if streams.state(column) != Some(Pending::Fetching) {
            continue;
        }
        if hot.lights(column) {
            streams.set(column, Pending::Queued);
            continue;
        }
        streams.pending.remove(&column);
        sent_lights.note(sections, column, &stored, Unkept::Keep);
        taken.push(ReadyColumn {
            column,
            stored,
            light: stored_light(light_gated.is_some(), has_sky_light),
            packet: None,
        });
    }
    encode_ready(&mut taken, direct_bits, &mut versions);
}

/// Sends a column that waits to be taken from the dimension back to the queue once the
/// dimension stops lighting it, so the data layer makes it instead.
fn requeue_unlit_columns(
    mut dims: Query<(&mut ColumnStreams, Ref<HotColumns>)>,
    mut unlit: Local<Vec<ColumnPos>>,
) {
    for (mut streams, hot) in &mut dims {
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
            streams.set(column, Pending::Queued);
        }
    }
}

/// Starts the work of each wanted column: the dimension waits for a column that it lights,
/// and the data layer makes the others.
fn raise_wanted_columns(
    mut dims: Query<(Entity, &mut ColumnStreams, &HotColumns)>,
    store: Res<ColumnStoreLink>,
    mut production: ResMut<ColumnProduction>,
    mut queued: Local<Vec<ColumnPos>>,
) {
    for (dim, mut streams, hot) in &mut dims {
        streams.in_state(Pending::Queued, &mut queued);
        for column in queued.drain(..) {
            if hot.lights(column) {
                streams.set(column, Pending::Awaiting);
            } else {
                streams.set(column, Pending::Fetching);
                production.want(dim, column, &store);
            }
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

/// The [`SentLight`] of the sections of a dimension, kept as the views get whole columns.
#[derive(SystemParam)]
pub(crate) struct SentLights<'w, 's> {
    commands: Commands<'w, 's>,
    sent: Query<'w, 's, &'static mut SentLight>,
}

/// What a whole column does to a section that keeps no [`SentLight`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unkept {
    /// The light of the column is not the light of the dimension, so the section keeps it.
    Keep,
    /// The light of the column is the light of the dimension, which sends every later change
    /// of it, so the section needs none.
    Skip,
}

impl SentLights<'_, '_> {
    /// Keeps on each section of `column` that the dimension holds the light that the views get
    /// with `stored`.
    fn note(
        &mut self,
        sections: &SectionIndex,
        column: ColumnPos,
        stored: &StoredColumn,
        unkept: Unkept,
    ) {
        for (y, section) in (stored.min_section_y..).zip(&stored.sections) {
            let Some(entity) = sections.get(SectionPos::new(column.x, y, column.z)) else {
                continue;
            };
            let block = section.block_light.as_ref().map(|light| light.0.clone());
            let sky = section.sky_light.as_ref().map(|light| light.0.clone());
            match self.sent.get_mut(entity) {
                Ok(mut sent) => {
                    sent.block = block;
                    sent.sky = sky;
                }
                Err(_) if unkept == Unkept::Keep => {
                    self.commands
                        .entity(entity)
                        .try_insert(SentLight { block, sky });
                }
                Err(_) => {}
            }
        }
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
    mut dims: Query<(
        &ColumnIndex,
        &SectionIndex,
        &mut ColumnStreams,
        Has<HasSkyLight>,
    )>,
    snapshots: ColumnSnapshots,
    light_gated: Option<Res<LightGated>>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut versions: MessageWriter<ColumnVersion>,
    traces: Option<Res<ColumnTraceLog>>,
    mut sent_lights: SentLights,
    mut ready: Local<Vec<ColumnPos>>,
    mut taken: Local<Vec<ReadyColumn>>,
) {
    let direct_bits = DirectBits::of(&blocks, &registries);
    for (column_index, sections, mut streams, has_sky_light) in &mut dims {
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
            sent_lights.note(sections, column, &stored, Unkept::Skip);
            streams.lit_this_tick.insert(column);
            streams.pending.remove(&column);
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
