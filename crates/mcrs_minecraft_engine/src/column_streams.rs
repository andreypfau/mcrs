use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::lifecycle::Discard;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{
    Added, Changed, Component, Has, Local, On, Or, Populated, Query, Resource, Single, With,
    Without, on_message,
};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemCondition, SystemSet};
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

use crate::bus::PacketPayload;
use crate::column_change::ColumnChange;
use crate::column_packet::{DirectBits, StoredLight, encode_stored_column};
use crate::column_production::ColumnProduction;
use crate::column_request::ColumnRequest;
use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::{ColumnStoreLink, Delivery, ViewedColumn};
use crate::column_version::ColumnVersion;
use crate::held_light::{HeldLight, RelitSections};
use crate::hot_ticket::CellCrossing;
use crate::light_codec::{build_delta_light_data, build_stored_light_data};
use crate::player_view::{PlayerView, ViewerPosition};
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

pub struct ColumnStreamPlugin;

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
                    take_column_requests.run_if(column_requests_pending),
                    report_player_views,
                    land_viewed_columns,
                    requeue_unlit_columns,
                    raise_wanted_columns.run_if(columns_queued),
                )
                    .chain()
                    .before(ChunkSpawnSet),
                project_ready_columns
                    .run_if(
                        on_message::<SectionStageChanged>
                            .or_eager(on_message::<SectionRelit>)
                            .or_eager(columns_awaited),
                    )
                    .after(raise_wanted_columns)
                    .before(ColumnLightGate),
                (
                    take_lit_columns.run_if(on_message::<ColumnLightSettled>),
                    publish_ready_columns.run_if(columns_ready),
                )
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
        app.add_message::<ViewerPosition>();
        app.add_message::<ColumnVersion>();
        app.add_message::<ColumnChange>();
        app.add_observer(close_player_view);
    }
}

#[derive(Resource, Default)]
pub struct ColumnRequests(pub Vec<ColumnRequest>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Queued,
    Fetching,
    Awaiting,
    Lighting,
    Ready,
}

#[derive(Component, Default)]
pub struct ColumnStreams {
    subscribed: FxHashSet<ColumnPos>,
    pending: FxHashMap<ColumnPos, Pending>,
    held: FxHashMap<ColumnPos, HeldLight>,
    relit: FxHashMap<ColumnPos, RelitSections>,
    awaited: Vec<ColumnPos>,
    queued: Vec<ColumnPos>,
    ready: Vec<ColumnPos>,
}

impl ColumnStreams {
    #[cfg(feature = "world-access")]
    pub fn subscribed_to(columns: impl IntoIterator<Item = ColumnPos>) -> Self {
        Self {
            subscribed: columns.into_iter().collect(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn is_subscribed(&self, column: ColumnPos) -> bool {
        self.subscribed.contains(&column)
    }

    pub(crate) fn holds(&self, column: ColumnPos) -> bool {
        self.is_subscribed(column) && !self.pending.contains_key(&column)
    }

    fn state(&self, column: ColumnPos) -> Option<Pending> {
        self.pending.get(&column).copied()
    }

    fn take_in_state(
        pending: &FxHashMap<ColumnPos, Pending>,
        queue: &mut Vec<ColumnPos>,
        state: Pending,
        columns: &mut Vec<ColumnPos>,
    ) {
        columns.append(queue);
        columns.sort_unstable();
        columns.dedup();
        columns.retain(|column| pending.get(column) == Some(&state));
    }

    fn take_queued(&mut self, columns: &mut Vec<ColumnPos>) {
        Self::take_in_state(&self.pending, &mut self.queued, Pending::Queued, columns);
    }

    fn take_ready(&mut self, columns: &mut Vec<ColumnPos>) {
        Self::take_in_state(&self.pending, &mut self.ready, Pending::Ready, columns);
    }

    fn set(&mut self, column: ColumnPos, next: Pending) {
        self.pending.insert(column, next);
        match next {
            Pending::Queued => self.queued.push(column),
            Pending::Ready => self.ready.push(column),
            Pending::Fetching | Pending::Awaiting | Pending::Lighting => {}
        }
    }

    fn forget(&mut self, column: ColumnPos) {
        self.subscribed.remove(&column);
        self.pending.remove(&column);
        self.held.remove(&column);
        self.relit.remove(&column);
    }

    fn sent(&mut self, column: ColumnPos, stored: &StoredColumn) {
        self.pending.remove(&column);
        self.relit.remove(&column);
        self.held.insert(column, HeldLight::of(stored));
    }
}

fn column_requests_pending(requests: Res<ColumnRequests>) -> bool {
    !requests.0.is_empty()
}

fn columns_queued(dims: Query<&ColumnStreams>) -> bool {
    dims.iter().any(|streams| !streams.queued.is_empty())
}

fn columns_ready(dims: Query<&ColumnStreams>) -> bool {
    dims.iter().any(|streams| !streams.ready.is_empty())
}

fn columns_awaited(dims: Query<&ColumnStreams>) -> bool {
    dims.iter().any(|streams| !streams.awaited.is_empty())
}

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

pub(crate) fn report_player_views(
    mut players: Populated<
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
    mut positions: MessageWriter<ViewerPosition>,
) {
    for (sent_view, transform, distance, in_dim, owner) in &mut players {
        positions.write(ViewerPosition {
            session: owner.0,
            position: transform.translation,
        });
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
        if !sent_view
            .map_unchanged(|sent| &mut sent.0)
            .set_if_neq(Some(view))
        {
            continue;
        }
        views.write(PlayerView {
            session: owner.0,
            view: Some(view),
        });
    }
}

fn close_player_view(
    discard: On<Discard<SentChunkView>>,
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
        payload: PacketPayload::LightUpdate(ClientboundLightUpdate {
            x: VarInt(column.x),
            z: VarInt(column.z),
            light_data,
        }),
        except: None,
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only for a column that is ready in this tick, and the encoded packet moves into the outbound column message"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(15))]
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
                    packet: None,
                });
            }
        }
    }
    let light = stored_light(light_gated.is_some(), has_sky_light);
    encode_ready(&mut taken, direct_bits, light, &mut versions);
}

fn requeue_unlit_columns(
    mut dims: Populated<(&mut ColumnStreams, &ColumnLoading), Changed<ColumnLoading>>,
    mut unlit: Local<Vec<ColumnPos>>,
) {
    for (mut streams, loading) in &mut dims {
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

fn raise_wanted_columns(
    mut dims: Query<(&mut ColumnStreams, &ColumnLoading)>,
    store: Res<ColumnStoreLink>,
    mut production: ResMut<ColumnProduction>,
    mut queued: Local<Vec<ColumnPos>>,
) {
    for (mut streams, loading) in &mut dims {
        streams.take_queued(&mut queued);
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
    mut relit_held: Local<Vec<ColumnPos>>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    let Some(dims) = dims else {
        return;
    };
    let (chunk_index, column_index, rows, mut streams) = dims.into_inner();
    for change in relit.read() {
        let Ok(pos) = positions.get(change.section) else {
            continue;
        };
        let column = ColumnPos::from(*pos);
        if streams.holds(column) {
            streams.relit.entry(column).or_default().note(change);
            relit_held.push(column);
        } else {
            candidates.push(column);
        }
    }
    relit_held.sort_unstable();
    relit_held.dedup();
    awaiting_light.write_batch(relit_held.drain(..).map(ColumnAwaitsLight));
    candidates.append(&mut streams.awaited);
    for change in changes.read().filter(|change| change.landed()) {
        candidates.extend(ColumnPos::from(change.pos).square(1));
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

fn take_lit_columns(
    mut settled: MessageReader<ColumnLightSettled>,
    dims: Option<
        Single<(
            &ColumnIndex,
            &DimensionTypeConfig,
            Has<HasSkyLight>,
            &mut ColumnStreams,
        )>,
    >,
    light: Query<(&SectionPos, &BlockLight, &SkyLight)>,
    mut changes: MessageWriter<ColumnChange>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    let Some(dims) = dims else {
        return;
    };
    let (column_index, rows, has_sky_light, mut streams) = dims.into_inner();
    let min_section_y = rows.min_y >> SectionPos::BITS;
    for &ColumnLightSettled(column) in settled.read() {
        if streams.state(column) == Some(Pending::Lighting) {
            column_trace::mark(traces.as_deref(), column, ColumnStage::Ready);
            streams.set(column, Pending::Ready);
            continue;
        }
        let Some(relit) = streams.relit.remove(&column) else {
            continue;
        };
        if !column_index.0.contains_key(&column) || !streams.holds(column) {
            continue;
        }
        let mut unheld = relit.take_unheld(streams.held.get_mut(&column), &light);
        if unheld.is_empty() {
            continue;
        }
        let light_data = build_delta_light_data(
            min_section_y,
            has_sky_light,
            &mut unheld.block,
            &mut unheld.sky,
        );
        changes.write(light_change(column, light_data));
    }
}

pub(crate) struct ReadyColumn {
    column: ColumnPos,
    stored: StoredColumn,
    packet: Option<Bytes>,
}

fn encode_ready(
    ready: &mut Vec<ReadyColumn>,
    direct_bits: DirectBits,
    light: StoredLight,
    versions: &mut MessageWriter<ColumnVersion>,
) {
    let encode_all = |_, ready: &mut [ReadyColumn]| {
        for column in ready {
            column.packet = encode_stored_column(column.column, &column.stored, direct_bits, light);
        }
    };
    let pool = ComputeTaskPool::try_get();
    let threads = pool.map_or(1, |pool| pool.thread_num()).max(1);
    let mut columns = ready.as_mut_slice();
    match pool.filter(|_| columns.len() > 1 && threads > 1) {
        Some(pool) => {
            let chunk = columns.len().div_ceil(threads * 4).max(1);
            columns.par_chunk_map_mut(pool, chunk, encode_all);
        }
        None => encode_all(0, columns),
    }
    versions.write_batch(ready.drain(..).filter_map(|ready| {
        Some(ColumnVersion {
            column: ready.column,
            packet: ready.packet?,
        })
    }));
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only for a column that is ready in this tick, and the encoded packet moves into the outbound column message"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(18))]
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
        streams.take_ready(&mut ready);
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
                packet: None,
            });
        }
        let light = stored_light(light_gated.is_some(), has_sky_light);
        encode_ready(&mut taken, direct_bits, light, &mut versions);
    }
}
