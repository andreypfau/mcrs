use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Discard;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{
    Added, Changed, Component, ContainsEntity, Has, Local, On, Or, Query, With,
};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Res, ResMut, SystemParam};
use bevy_tasks::{ComputeTaskPool, ParallelSliceMut};
use bevy_utils::Parallel;
use bytes::Bytes;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::chunk_view::{
    ChunkTrackingView, PlayerViewDistance, SentChunkView,
};
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::{
    DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{
    ChunkSpawnSet, MAX_SPAWNS_PER_TICK, SectionEntitySet,
};
use mcrs_minecraft_engine_world::world::lifecycle::trace as column_trace;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, ColumnTraceLog};
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, ColumnLightSettled, LightGated, SectionRelit,
};
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, SectionBlockEntities,
};
use mcrs_minecraft_engine_world::world::storage::column::{
    ColumnIndex, ColumnLifecycleSet, ColumnPos as EngineColumnPos,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_heightmap::{MotionHeightmap, NoLeavesHeightmap, SurfaceHeightmap};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::chunk::ChunkDataBlockEntity;
use mcrs_minecraft_protocol_game::clientbound::ClientboundChunkBatchFinished;
use mcrs_minecraft_protocol_game::clientbound::ClientboundChunkBatchStart;
use mcrs_minecraft_protocol_game::clientbound::ClientboundChunkCacheRadius;
use mcrs_minecraft_protocol_game::clientbound::ClientboundForgetLevelChunk;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetChunkCacheCenter;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChunkBatchReceived;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::SectionPos;
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{trace, warn};

use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::column_held::ColumnHeld;
use crate::column_packet::{
    ColumnParts, DirectBits, StoredLight, encode_packet, encode_sections, encode_stored_column,
    push_block_entity,
};
use crate::column_store::{ColumnStoreLink, ViewedColumn};
use crate::column_waiters::ColumnWaiters;
use crate::heightmap::client_heightmaps;
use crate::hot_ticket::HotColumns;
use crate::light_codec::{LightCodecParams, build_full_light_data, build_fullbright_light_data};

pub struct ColumnViewPlugin;

/// Applies the view, turns what it wants into tickets, and sends the columns that are ready.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnViewSet;

impl Plugin for ColumnViewPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            ColumnViewSet
                .in_set(TickSet::Project)
                .after(ColumnLifecycleSet)
                .before(SectionEntitySet),
        );
        app.add_systems(
            FixedUpdate,
            (
                // The tickets must precede the spawn that follows in the same tick.
                (
                    release_sent_tickets,
                    update_view,
                    land_viewed_columns,
                    raise_queued_columns,
                )
                    .chain()
                    .before(ChunkSpawnSet),
                project_ready_columns
                    .after(raise_queued_columns)
                    .before(ColumnLightGate),
                (
                    promote_lit_columns,
                    send_column_queue,
                    crate::column_held::announce_spawned_columns,
                    crate::column_held::mirror_held_columns,
                )
                    .chain()
                    .after(ColumnLightGate),
            )
                .in_set(ColumnViewSet),
        );
        app.register_required_components::<ColumnLoading, ColumnWaiters>();
        app.add_message::<ColumnAwaitsLight>();
        app.add_message::<ColumnLightSettled>();
        app.add_message::<SectionRelit>();

        app.add_observer(handle_batch_acknowledgement);
        app.add_observer(release_loading_tickets);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColumnState {
    /// In the view, with no ticket raised for it yet.
    Queued,
    /// Cold, waiting for the data layer to hand back its last version.
    Fetching,
    /// Ticketed, waiting for its sections to land.
    Awaiting,
    /// Landed and lit, waiting for the light around it to settle.
    Lighting,
    Ready,
    /// Cold, with its last version from the data layer encoded and ready to send.
    Stored,
    Sent,
}

impl ColumnState {
    /// The states a column leaves only on news about the column itself, which
    /// [`ColumnWaiters`] routes to the players in them.
    fn waits(self) -> bool {
        matches!(
            self,
            Self::Fetching | Self::Awaiting | Self::Lighting | Self::Stored
        )
    }

    /// Whether the view holds a loading ticket on a column in this state. With a data layer the
    /// view lets the ticket go once the column is sent, so a column outside every hot area
    /// goes cold.
    fn holds_ticket(self, views: ViewSource) -> bool {
        match views {
            ViewSource::Dimension => self != Self::Queued,
            ViewSource::DataLayer => matches!(self, Self::Awaiting | Self::Lighting | Self::Ready),
        }
    }
}

/// Where the views of a dimension get the columns that are not loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewSource {
    /// The view loads every column it holds, and keeps it loaded.
    Dimension,
    /// The view gets a cold column from the data layer.
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

/// What a player is owed, column by column, as the player's [`SentChunkView`] decided it. A
/// column the view wants has exactly one state, so this is the view's want list and no second
/// copy of it is kept.
///
/// No pending state is ordered. A batch takes the columns nearest the player's position at the
/// moment it goes out, so any order stored earlier could only be a stale one — the player turns,
/// and the front of the queue is behind them.
#[derive(Component)]
#[require(SentChunkView)]
pub struct ColumnView {
    states: FxHashMap<ColumnPos, ColumnState>,
    /// The unsent columns of `states` by state, so a send walks what is left to do rather than
    /// the whole view. Kept by `set` alone.
    unsent: [FxHashSet<ColumnPos>; 6],
    batch_quota: f32,
    desired_columns_per_tick: f32,
    unacknowledged_batches: u32,
    max_unacknowledged_batches: u32,
}

impl Default for ColumnView {
    fn default() -> Self {
        Self {
            states: FxHashMap::default(),
            unsent: Default::default(),
            batch_quota: 0.0,
            desired_columns_per_tick: START_COLUMNS_PER_TICK,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: 1,
        }
    }
}

impl ColumnView {
    /// A view that already holds `columns` on the client, for a player that has to observe
    /// columns without the send path behind it.
    pub fn holding(columns: impl IntoIterator<Item = ColumnPos>) -> Self {
        let mut view = Self::default();
        view.states.extend(
            columns
                .into_iter()
                .map(|column| (column, ColumnState::Sent)),
        );
        view
    }

    pub fn holds(&self, column: ColumnPos) -> bool {
        self.state(column) == Some(ColumnState::Sent)
    }

    /// Every column the client holds.
    pub fn held(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.states
            .iter()
            .filter(|(_, state)| **state == ColumnState::Sent)
            .map(|(column, _)| *column)
    }

    fn state(&self, column: ColumnPos) -> Option<ColumnState> {
        self.states.get(&column).copied()
    }

    /// Grows the batch quota by what the client asks for each call, up to one batch, while the
    /// client has room for another batch.
    fn refill_quota(&mut self) {
        if self.unacknowledged_batches >= self.max_unacknowledged_batches {
            return;
        }
        let desired = self.desired_columns_per_tick;
        self.batch_quota = (self.batch_quota + desired).min(desired.max(1.0));
    }

    fn can_send(&self) -> bool {
        self.unacknowledged_batches < self.max_unacknowledged_batches
            && self.batch_quota >= 1.0
            && self.sendable().next().is_some()
    }

    fn sendable(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.in_state(ColumnState::Ready)
            .chain(self.in_state(ColumnState::Stored))
    }

    fn in_state(&self, state: ColumnState) -> impl Iterator<Item = ColumnPos> + '_ {
        let [queued, fetching, awaiting, lighting, ready, stored] = &self.unsent;
        let unsent = match state {
            ColumnState::Queued => Some(queued),
            ColumnState::Fetching => Some(fetching),
            ColumnState::Awaiting => Some(awaiting),
            ColumnState::Lighting => Some(lighting),
            ColumnState::Ready => Some(ready),
            ColumnState::Stored => Some(stored),
            ColumnState::Sent => None,
        };
        unsent
            .into_iter()
            .flat_map(|columns| columns.iter().copied())
    }

    fn unsent_mut(&mut self, state: ColumnState) -> Option<&mut FxHashSet<ColumnPos>> {
        let [queued, fetching, awaiting, lighting, ready, stored] = &mut self.unsent;
        match state {
            ColumnState::Queued => Some(queued),
            ColumnState::Fetching => Some(fetching),
            ColumnState::Awaiting => Some(awaiting),
            ColumnState::Lighting => Some(lighting),
            ColumnState::Ready => Some(ready),
            ColumnState::Stored => Some(stored),
            ColumnState::Sent => None,
        }
    }

    /// Moves `column` of `player`'s view to `state`, and keeps `waiters`, the index of the
    /// view's dimension, in step with it.
    fn set(
        &mut self,
        waiters: &mut ColumnWaiters,
        player: Entity,
        column: ColumnPos,
        state: Option<ColumnState>,
    ) -> Option<ColumnState> {
        let previous = match state {
            Some(state) => self.states.insert(column, state),
            None => self.states.remove(&column),
        };
        if let Some(columns) = previous.and_then(|previous| self.unsent_mut(previous)) {
            columns.remove(&column);
        }
        if let Some(columns) = state.and_then(|state| self.unsent_mut(state)) {
            columns.insert(column);
        }
        match (
            previous.is_some_and(ColumnState::waits),
            state.is_some_and(ColumnState::waits),
        ) {
            (true, false) => waiters.remove(column, player),
            (false, true) => waiters.add(column, player),
            (true, true) | (false, false) => {}
        }
        match (previous, state) {
            (Some(ColumnState::Stored), Some(ColumnState::Stored)) => {}
            (Some(ColumnState::Stored), _) => waiters.release_packet(column),
            (_, Some(ColumnState::Stored)) => waiters.hold_packet(column),
            _ => {}
        }
        previous
    }

    fn waiting(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.in_state(ColumnState::Fetching)
            .chain(self.in_state(ColumnState::Awaiting))
            .chain(self.in_state(ColumnState::Lighting))
            .chain(self.in_state(ColumnState::Stored))
    }

    /// The client has taken a batch and says how many columns a tick it managed while doing so.
    pub fn acknowledge_batch(&mut self, desired_columns_per_tick: f32) {
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_sub(1);
        self.desired_columns_per_tick = if desired_columns_per_tick.is_nan() {
            MIN_COLUMNS_PER_TICK
        } else {
            desired_columns_per_tick.clamp(MIN_COLUMNS_PER_TICK, MAX_COLUMNS_PER_TICK)
        };
        if self.unacknowledged_batches == 0 {
            self.batch_quota = 1.0;
        }
        self.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;
    }
}

fn handle_batch_acknowledgement(
    on: On<Received<ServerboundChunkBatchReceived>>,
    mut players: Query<&mut ColumnView>,
) {
    let Some(ack) = on.packet() else {
        return;
    };
    let Ok(mut chunk_view) = players.get_mut(on.entity) else {
        return;
    };
    chunk_view.acknowledge_batch(ack.desired_chunks_per_tick);
}

/// Applies the player's view: tells the client where it is, queues the columns the view reveals,
/// and lets go of the ones it leaves. The forget goes out from the same view diff that sent the
/// column, as vanilla's chunk map does, so what the client holds is exactly what the server
/// counts as sent: a forget from any other radius leaves a column the server will never send
/// again.
pub(crate) fn update_view(
    mut players: Query<
        (
            Entity,
            &mut ColumnView,
            &mut SentChunkView,
            &Transform,
            &PlayerViewDistance,
            &InDimension,
            &Owner,
        ),
        Or<(
            Changed<Transform>,
            Changed<PlayerViewDistance>,
            Added<ColumnView>,
        )>,
    >,
    mut dims: Query<(&mut ColumnLoading, &mut ColumnWaiters, &DimensionTypeConfig)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut held: MessageWriter<ColumnHeld>,
    mut left: Local<Vec<ColumnPos>>,
    traces: Option<Res<ColumnTraceLog>>,
    store: Option<Res<ColumnStoreLink>>,
) {
    let views = ViewSource::of(store.as_deref());
    for (player, mut chunk_view, mut sent_view, transform, distance, in_dim, owner) in &mut players
    {
        let Ok((mut loading, mut waiters, type_config)) = dims.get_mut(in_dim.entity()) else {
            continue;
        };
        let min_section_y = type_config.min_y >> SectionPos::BITS;
        let new_view = ChunkTrackingView::with_y_bounds(
            SectionPos::from(transform.translation),
            distance.distance + 1,
            distance.vert_distance + 1,
            min_section_y,
            min_section_y + type_config.section_count as i32 - 1,
        );
        let old_view = sent_view.0;
        if old_view == Some(new_view) {
            continue;
        }
        send_cache_view(old_view, new_view, owner.0, &mut packet_writer);

        let view = &mut *chunk_view;
        let waiters = &mut *waiters;
        let mut queue = |column: ColumnPos| {
            if view.state(column).is_none() {
                view.set(waiters, player, column, Some(ColumnState::Queued));
            }
        };
        match old_view {
            None => new_view.each_column(queue),
            Some(old_view) => {
                ChunkTrackingView::diff_columns(&old_view, &new_view, &mut queue, |column| {
                    left.push(column)
                })
            }
        }

        for column in left.drain(..) {
            column_trace::forget(traces.as_deref(), column);
            let Some(state) = view.set(waiters, player, column, None) else {
                continue;
            };
            if state.holds_ticket(views) {
                loading.remove_ticket(column);
            }
            if state == ColumnState::Sent {
                held.write(ColumnHeld {
                    player,
                    dim: in_dim.entity(),
                    column,
                    held: false,
                });
                packet_writer.write(
                    to(
                        owner.0,
                        PacketPayload::ChunkUnload(ClientboundForgetLevelChunk {
                            x: column.x,
                            z: column.z,
                        }),
                    )
                    .critical(),
                );
            }
        }
        sent_view.0 = Some(new_view);
    }
}

/// Cache center and radius, sent Critical: the client renders nothing without them.
fn send_cache_view(
    old_view: Option<ChunkTrackingView>,
    new_view: ChunkTrackingView,
    session: PlayerSession,
    packet_writer: &mut MessageWriter<OutboundPlayerPacket>,
) {
    if old_view.is_none_or(|old_view| old_view.center != new_view.center) {
        packet_writer.write(
            to(
                session,
                PacketPayload::SetChunkCacheCenter(ClientboundSetChunkCacheCenter {
                    x: VarInt(new_view.center.x),
                    z: VarInt(new_view.center.z),
                }),
            )
            .critical(),
        );
    }
    if old_view.is_none_or(|old_view| old_view.distance != new_view.distance) {
        packet_writer.write(
            to(
                session,
                PacketPayload::SetChunkCacheRadius(ClientboundChunkCacheRadius {
                    radius: VarInt(new_view.distance as i32),
                }),
            )
            .critical(),
        );
    }
}

/// Raises what the view wants, nearest first, holding a loading ticket on each column it raises.
///
/// The budget is columns rather than sections: raising a column costs a whole column's worth of
/// spawns, so counting sections here would let the view ask for more than `spawn_chunks` can
/// ever hand out and grow the queue without bound.
pub(crate) fn raise_queued_columns(
    mut players: Query<(Entity, &mut ColumnView, &SentChunkView, &InDimension)>,
    mut dims: Query<(&mut ColumnLoading, &mut ColumnWaiters, &DimensionTypeConfig)>,
    mut nearest: Local<Vec<ColumnPos>>,
    traces: Option<Res<ColumnTraceLog>>,
    mut store: Option<ResMut<ColumnStoreLink>>,
    hot_columns: Query<&HotColumns>,
) {
    for (player, mut chunk_view, &SentChunkView(view), in_dim) in &mut players {
        let hot = hot_columns.get(in_dim.entity()).ok();
        let Some(view) = view else {
            continue;
        };
        nearest.extend(chunk_view.in_state(ColumnState::Queued));
        if nearest.is_empty() {
            continue;
        }
        let Ok((mut loading, mut waiters, type_config)) = dims.get_mut(in_dim.entity()) else {
            nearest.clear();
            continue;
        };
        let budget = (MAX_SPAWNS_PER_TICK / type_config.section_count.max(1) as usize).max(1);
        let center = ColumnPos::new(view.center.x, view.center.z);
        if budget < nearest.len() {
            nearest.select_nth_unstable_by_key(budget, |column| column.distance_squared(center));
            nearest.truncate(budget);
        }
        nearest.sort_unstable_by_key(|column| column.distance_squared(center));

        for column in nearest.drain(..) {
            if let Some(store) = store.as_deref_mut()
                && store.may_hold(column)
                && hot.is_some_and(|hot| !hot.is_hot(column))
            {
                chunk_view.set(&mut waiters, player, column, Some(ColumnState::Fetching));
                store.view(in_dim.entity(), column);
                continue;
            }
            chunk_view.set(&mut waiters, player, column, Some(ColumnState::Awaiting));
            loading.add_ticket(column);
            column_trace::mark(traces.as_deref(), column, ColumnStage::Ticketed);
        }
    }
}

/// Lets go of the loading tickets that the sends of the last tick handed back. The send runs
/// apart from the tickets, so it encodes while the sections spawn.
fn release_sent_tickets(mut dims: Query<(&mut ColumnLoading, &mut ColumnWaiters)>) {
    for (mut loading, mut waiters) in &mut dims {
        for column in waiters.take_released_tickets() {
            loading.remove_ticket(column);
        }
    }
}

/// Moves each view that waits on a column the data layer answered for: to send the version the
/// data layer held, or to load the column when the data layer held none.
///
/// The packet is encoded once for every view that waits on the column. A column that went hot
/// in the meantime is taken from the dimension instead, so a version older than the hot one
/// never reaches a view.
pub(crate) fn land_viewed_columns(
    mut store: Option<ResMut<ColumnStoreLink>>,
    mut players: Query<&mut ColumnView>,
    mut dims: Query<(
        &mut ColumnLoading,
        &mut ColumnWaiters,
        &HotColumns,
        Has<HasSkyLight>,
    )>,
    light_gated: Option<Res<LightGated>>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut fetching: Local<Vec<Entity>>,
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
        let Ok((mut loading, mut waiters, hot_columns, has_sky_light)) = dims.get_mut(dim) else {
            continue;
        };
        fetching.extend(waiters.of(column).iter().copied().filter(|player| {
            players
                .get(*player)
                .is_ok_and(|view| view.state(column) == Some(ColumnState::Fetching))
        }));
        if fetching.is_empty() {
            continue;
        }
        let light = match light_gated {
            Some(_) => StoredLight::Kept { has_sky_light },
            None => StoredLight::Fullbright,
        };
        let hot = hot_columns.is_hot(column);
        let packet = stored
            .filter(|_| !hot)
            .and_then(|stored| encode_stored_column(column, &stored, direct_bits, light));
        let next = match &packet {
            _ if hot => ColumnState::Queued,
            Some(_) => ColumnState::Stored,
            None => ColumnState::Awaiting,
        };
        if let Some(packet) = packet {
            waiters.insert_packet(column, packet);
        }
        for player in fetching.drain(..) {
            let Ok(mut view) = players.get_mut(player) else {
                continue;
            };
            view.set(&mut waiters, player, column, Some(next));
            if next == ColumnState::Awaiting {
                loading.add_ticket(column);
            }
        }
    }
}

/// Only the view knows which sections it ticketed, so a player leaving the
/// dimension that does not hand them back here pins them loaded for good.
/// A player that changes dimension leaves its old view behind with the entity
/// this despawns, so the same path takes it out of the old dimension's waiters.
fn release_loading_tickets(
    discard: On<Discard, ColumnView>,
    players: Query<(&ColumnView, &InDimension)>,
    mut dims: Query<(&mut ColumnLoading, &mut ColumnWaiters)>,
    store: Option<Res<ColumnStoreLink>>,
) {
    let views = ViewSource::of(store.as_deref());
    let player = discard.event().entity;
    let Ok((view, in_dim)) = players.get(player) else {
        return;
    };
    let Ok((mut loading, mut waiters)) = dims.get_mut(in_dim.entity()) else {
        return;
    };
    for (&column, state) in &view.states {
        if state.holds_ticket(views) {
            loading.remove_ticket(column);
        }
        if *state == ColumnState::Stored {
            waiters.release_packet(column);
        }
    }
    for column in view.waiting() {
        waiters.remove(column, player);
    }
}

/// The column entity and the sections it hands the client, in client order, or `None` while
/// the engine is still short of either. The single place a column's presence is decided, so
/// readiness and the send that follows it cannot disagree about what the column is made of.
///
/// The column entity is part of that answer, not a detail of the send: light and heightmaps
/// are read off it, and a column sent before `reconcile_columns` has indexed it would carry
/// neither. It goes out once and never again, so it would stay black.
fn resolve_column(
    chunk_index: &SectionIndex,
    column_index: &ColumnIndex,
    col: ColumnPos,
    section_count: i32,
    off: i32,
) -> Option<(Entity, Vec<Entity>)> {
    let column = column_index
        .0
        .get(&EngineColumnPos::new(col.x, col.z))
        .map(|slot| slot.entity)?;
    let sections = (0..section_count)
        .map(|client_y| chunk_index.get(SectionPos::new(col.x, client_y - off, col.z)))
        .collect::<Option<Vec<_>>>()?;
    Some((column, sections))
}

/// A column of a player's view.
#[derive(Clone, Copy)]
pub(crate) struct PlayerColumn {
    player: Entity,
    column: ColumnPos,
}

/// Moves a column on from "wanted" once every section it carries has landed: to ready, or in a
/// light-gated world to waiting for the light around it to settle.
///
/// The light gate belongs before the send: a column goes out once and never again, so sending
/// one before its light has settled leaves it permanently black on the client. Readiness
/// depends on the column alone, so each column is decided once and moves on in every view of
/// its dimension that waits for it.
#[expect(
    clippy::too_many_arguments,
    reason = "a system takes its params as arguments"
)]
pub(crate) fn project_ready_columns(
    mut players: Query<&mut ColumnView>,
    mut dims: Query<(
        &SectionIndex,
        &ColumnIndex,
        &DimensionTypeConfig,
        &mut ColumnWaiters,
    )>,
    chunks: Query<&SectionStage>,
    lit: Query<(), (With<BlockLight>, With<SkyLight>)>,
    light_gated: Option<Res<LightGated>>,
    mut awaiting_light: MessageWriter<ColumnAwaitsLight>,
    mut ready: Local<Vec<PlayerColumn>>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    let await_light = light_gated.is_some();
    for (chunk_index, column_index, type_config, mut waiters) in dims.iter_mut() {
        let section_count = type_config.section_count as i32;
        let off = offset_sections(type_config.min_y);
        let sections_of = |col: ColumnPos| {
            resolve_column(chunk_index, column_index, col, section_count, off).filter(
                |(_, sections)| {
                    sections
                        .iter()
                        .all(|&e| chunks.get(e) == Ok(&SectionStage::Loaded))
                },
            )
        };
        // A cell's light is decided by the blocks within fifteen of it, which
        // reaches one column out and no further. So the neighbours owe this
        // column their blocks, never their light: waiting on their light too
        // would hold every column of a bulk load behind the whole queue.
        let blocks_landed = |col: ColumnPos| sections_of(col).is_some();
        let light_landed = |col: ColumnPos| {
            sections_of(col).is_some_and(|(_, sections)| {
                !await_light || sections.iter().all(|&chunk_e| lit.contains(chunk_e))
            })
        };
        // Light computed against a neighbour whose blocks have not arrived
        // is a seam the column would carry for as long as the client holds
        // it, because a column goes out once. The light gate then waits for
        // the work those neighbours raised.
        let is_ready = |col: ColumnPos| light_landed(col) && margin_of(col).all(blocks_landed);
        let waiting: &ColumnWaiters = &waiters;
        let mut gather = |column: ColumnPos| {
            let awaits = |player: &Entity| {
                players
                    .get(*player)
                    .is_ok_and(|view| view.state(column) == Some(ColumnState::Awaiting))
            };
            if !waiting.of(column).iter().any(awaits) || !is_ready(column) {
                return;
            }
            let awaiting = waiting.of(column).iter().copied().filter(awaits);
            ready.extend(awaiting.map(|player| PlayerColumn { player, column }));
        };
        for column in waiting.columns() {
            gather(column);
        }
        ready.sort_unstable_by_key(|ready| ready.player);
        for batch in ready.chunk_by(|left, right| left.player == right.player) {
            let Some(player) = batch.first().map(|ready| ready.player) else {
                continue;
            };
            let Ok(mut chunk_view) = players.get_mut(player) else {
                continue;
            };
            for &PlayerColumn { column, .. } in batch {
                if await_light {
                    chunk_view.set(&mut waiters, player, column, Some(ColumnState::Lighting));
                    awaiting_light.write(ColumnAwaitsLight(column));
                } else {
                    trace!("Column {:?} ready", column);
                    column_trace::mark(traces.as_deref(), column, ColumnStage::Ready);
                    chunk_view.set(&mut waiters, player, column, Some(ColumnState::Ready));
                }
            }
        }
        ready.clear();
    }
}

/// Moves a column whose light has settled from waiting for it to ready, for every player
/// waiting on it.
pub(crate) fn promote_lit_columns(
    mut settled: MessageReader<ColumnLightSettled>,
    mut dims: Query<&mut ColumnWaiters>,
    mut players: Query<&mut ColumnView>,
    mut waiting: Local<Vec<Entity>>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    for &ColumnLightSettled(col) in settled.read() {
        for mut waiters in &mut dims {
            waiting.extend_from_slice(waiters.of(col));
            for player in waiting.drain(..) {
                let Ok(mut chunk_view) = players.get_mut(player) else {
                    continue;
                };
                if chunk_view.state(col) == Some(ColumnState::Lighting) {
                    trace!("Column {:?} ready", col);
                    column_trace::mark(traces.as_deref(), col, ColumnStage::Ready);
                    chunk_view.set(&mut waiters, player, col, Some(ColumnState::Ready));
                }
            }
        }
    }
}

/// The rate the client is asked to answer with, in columns a tick. The ceiling is only a guard
/// against a nonsense answer: a client on the same machine as its server measures itself able to
/// take several hundred a tick, and clamping that back to a round number throttles the load for
/// no reason the client asked for.
const MIN_COLUMNS_PER_TICK: f32 = 0.01;
const MAX_COLUMNS_PER_TICK: f32 = 4096.0;
const START_COLUMNS_PER_TICK: f32 = 9.0;

/// A ceiling on one batch, not the rate: the client's answer is what paces the load, and a cap
/// low enough to bind first silently overrides it — at a full render distance a column is some
/// seventy kilobytes on the wire, so half the socket's queue was barely a hundred of them. The
/// bridge hands a blob over in pieces and buffers what the writer cannot take, and only kills a
/// connection sixteen socket queues behind, so a batch this size leaves it several ticks of room.
const MAX_BATCH_BYTES: usize = 4 * mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET;

/// Batches allowed in flight once the client has answered one. Until then a single batch is
/// out at a time, so a client that cannot keep up is never sent a second one to prove it.
const MAX_UNACKNOWLEDGED_BATCHES: u32 = 10;

/// Squared XZ distance from a column to the centre of the player's view.
fn column_distance_sq(pos: ColumnPos, center: SectionPos) -> i64 {
    let dx = (pos.x - center.x) as i64;
    let dz = (pos.z - center.z) as i64;
    dx * dx + dz * dz
}

/// A column the next batch of a player takes, before it is encoded.
pub(crate) struct WantedColumn {
    player: Entity,
    session: PlayerSession,
    at: DimColumn,
    source: PacketSource,
}

/// Where the packet of a column comes from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PacketSource {
    /// Encoded from the sections the dimension holds.
    Dimension,
    /// Encoded once from the last version the data layer handed back.
    DataLayer,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct DimColumn {
    dim: Entity,
    column: ColumnPos,
}

enum ColumnEncoding {
    Pending,
    Encoded(Bytes),
    Skipped(ColumnSkip),
}

/// A column encoded once for every player whose batch takes it.
struct SharedColumn {
    at: DimColumn,
    encoding: ColumnEncoding,
}

/// The columns of one send, each encoded once. Kept between sends so its storage is reused.
#[derive(Default)]
pub(crate) struct SharedColumns {
    slots: FxHashMap<DimColumn, usize>,
    columns: Vec<SharedColumn>,
}

impl SharedColumns {
    fn clear(&mut self) {
        self.slots.clear();
        self.columns.clear();
    }

    fn want(&mut self, at: DimColumn) {
        let next = self.columns.len();
        self.slots.entry(at).or_insert_with(|| {
            self.columns.push(SharedColumn {
                at,
                encoding: ColumnEncoding::Pending,
            });
            next
        });
    }

    fn encoding(&self, at: DimColumn) -> Option<&ColumnEncoding> {
        let slot = self.slots.get(&at)?;
        self.columns.get(*slot).map(|column| &column.encoding)
    }
}

/// The sections, light, heightmaps and registries a column is encoded from.
#[derive(SystemParam)]
pub(crate) struct ColumnSource<'w, 's> {
    chunks: Query<
        'w,
        's,
        (
            &'static ChunkBlocks,
            &'static BiomePalette,
            Option<&'static SectionBlockEntities>,
        ),
    >,
    stages: Query<'w, 's, &'static SectionStage>,
    block_entities_held: Query<'w, 's, &'static BlockEntity>,
    dim_chunk_indexes: Query<'w, 's, &'static SectionIndex>,
    dim_column_indexes: Query<'w, 's, &'static ColumnIndex>,
    dim_type_configs: Query<'w, 's, &'static DimensionTypeConfig>,
    column_heightmaps: Query<
        'w,
        's,
        (
            &'static SurfaceHeightmap,
            &'static MotionHeightmap,
            &'static NoLeavesHeightmap,
        ),
    >,
    codec_params: LightCodecParams<'w, 's>,
    light_gated: Option<Res<'w, LightGated>>,
    block_definitions: Res<'w, Blocks>,
    registries: Res<'w, CurrentRegistries>,
    traces: Option<Res<'w, ColumnTraceLog>>,
}

/// What a player's batch is picked and sent with.
type Viewers = (
    Entity,
    &'static mut ColumnView,
    &'static SentChunkView,
    &'static InDimension,
    &'static Owner,
);

/// Chunk-load wire emit routed through the `OutboundPlayerPacket` bus, once a tick.
///
/// The rate is the client's: each batch is bracketed by a start and a finish, and the client
/// answers with the columns a tick it managed. Each tick refills every player's quota by that
/// answer. Chunks are sent at Critical priority so they are never dropped by the bridge.
///
/// The players pick their batches in parallel. Each column any batch takes is then encoded
/// once, in parallel, and every player that takes it receives the same buffer. The batches go
/// out ordered by player, so the order does not depend on the thread count.
#[expect(
    clippy::too_many_arguments,
    reason = "a system takes its params as arguments"
)]
pub(crate) fn send_column_queue(
    mut players: Query<Viewers>,
    mut dims: Query<(&mut ColumnWaiters, Option<&HotColumns>)>,
    store: Option<Res<ColumnStoreLink>>,
    source: ColumnSource,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut held: MessageWriter<ColumnHeld>,
    nearest: Local<Parallel<Vec<ColumnPos>>>,
    mut picked: Local<Parallel<Vec<WantedColumn>>>,
    mut wanted: Local<Vec<WantedColumn>>,
    mut shared: Local<SharedColumns>,
) {
    // Fanning out to the compute pool for nothing wakes every one of its threads.
    let mut any_can_send = false;
    for (_, mut chunk_view, ..) in &mut players {
        chunk_view.refill_quota();
        any_can_send |= chunk_view.can_send();
    }
    if !any_can_send {
        return;
    }
    players.par_iter().for_each(|viewer| {
        nearest.scope(|nearest| {
            picked.scope(|picked| pick_batch(viewer, nearest, picked));
        });
    });

    picked.drain_into(&mut wanted);
    send_picked(
        &source,
        &mut players,
        &mut dims,
        ViewSource::of(store.as_deref()),
        &mut wanted,
        &mut shared,
        &mut packet_writer,
        &mut held,
    );
}

/// Encodes each column of `wanted` once and sends every player its batch, ordered by player.
#[expect(
    clippy::too_many_arguments,
    reason = "the parts of the system that every batch is sent with"
)]
fn send_picked(
    source: &ColumnSource,
    players: &mut Query<Viewers>,
    dims: &mut Query<(&mut ColumnWaiters, Option<&HotColumns>)>,
    views: ViewSource,
    wanted: &mut Vec<WantedColumn>,
    shared: &mut SharedColumns,
    packet_writer: &mut MessageWriter<OutboundPlayerPacket>,
    held: &mut MessageWriter<ColumnHeld>,
) {
    // One thread picks all of a player's columns in order, so a stable sort keeps each batch
    // in the order it was picked.
    wanted.sort_by_key(|column| column.player);
    shared.clear();
    for column in wanted.iter() {
        if column.source == PacketSource::Dimension {
            shared.want(column.at);
        }
    }
    encode_shared(source, &mut shared.columns);

    for batch in wanted.chunk_by(|left, right| left.player == right.player) {
        let Some(first) = batch.first() else {
            continue;
        };
        let (Ok((_, mut chunk_view, ..)), Ok((mut waiters, hot))) =
            (players.get_mut(first.player), dims.get_mut(first.at.dim))
        else {
            continue;
        };
        send_batch(
            source,
            shared,
            &mut chunk_view,
            &mut DimSend {
                waiters: &mut waiters,
                hot,
                views,
            },
            batch,
            packet_writer,
            held,
        );
    }
    wanted.clear();
}

/// The dimension of a batch: its waiters, its loading tickets and where its views get cold
/// columns.
struct DimSend<'a> {
    waiters: &'a mut ColumnWaiters,
    hot: Option<&'a HotColumns>,
    views: ViewSource,
}

/// The packet that `wanted` goes out with, or `None` when it does not go out in this batch.
fn batch_packet(
    shared: &SharedColumns,
    chunk_view: &mut ColumnView,
    dim: &mut DimSend,
    wanted: &WantedColumn,
) -> Option<Bytes> {
    let column = wanted.at.column;
    match wanted.source {
        PacketSource::DataLayer => {
            // A column that went hot since the data layer answered may have changed since,
            // so the view takes it from the dimension instead.
            let packet = dim
                .waiters
                .packet(column)
                .filter(|_| !dim.hot.is_some_and(|hot| hot.is_hot(column)))
                .cloned();
            if packet.is_none() {
                chunk_view.set(
                    dim.waiters,
                    wanted.player,
                    column,
                    Some(ColumnState::Queued),
                );
            }
            packet
        }
        PacketSource::Dimension => match shared.encoding(wanted.at) {
            Some(ColumnEncoding::Encoded(packet)) => Some(packet.clone()),
            Some(ColumnEncoding::Skipped(ColumnSkip::LostSection)) => {
                chunk_view.set(
                    dim.waiters,
                    wanted.player,
                    column,
                    Some(ColumnState::Awaiting),
                );
                None
            }
            Some(ColumnEncoding::Skipped(ColumnSkip::Failed) | ColumnEncoding::Pending) | None => {
                None
            }
        },
    }
}

/// Turns the encoded columns of one player's pick into a bracketed batch, nearest first.
fn send_batch(
    source: &ColumnSource,
    shared: &SharedColumns,
    chunk_view: &mut ColumnView,
    dim: &mut DimSend,
    batch: &[WantedColumn],
    packet_writer: &mut MessageWriter<OutboundPlayerPacket>,
    held: &mut MessageWriter<ColumnHeld>,
) {
    let mut sends = 0i32;
    let mut batch_bytes = 0usize;
    for wanted in batch {
        if batch_bytes >= MAX_BATCH_BYTES {
            break;
        }
        let column_pos = wanted.at.column;
        let Some(packet) = batch_packet(shared, chunk_view, dim, wanted) else {
            continue;
        };
        if sends == 0 {
            packet_writer.write(
                to(
                    wanted.session,
                    PacketPayload::ChunkBatchStart(ClientboundChunkBatchStart),
                )
                .critical(),
            );
        }
        column_trace::mark(source.traces.as_deref(), column_pos, ColumnStage::Sent);
        let previous = chunk_view.set(
            dim.waiters,
            wanted.player,
            column_pos,
            Some(ColumnState::Sent),
        );
        if previous.is_some_and(|state| state.holds_ticket(dim.views))
            && !ColumnState::Sent.holds_ticket(dim.views)
        {
            dim.waiters.release_ticket(column_pos);
        }
        trace!(
            target: "mcrs_minecraft_engine::player",
            session = ?wanted.session,
            col_x = column_pos.x,
            col_z = column_pos.z,
            bytes = packet.len(),
            "send_column_queue: emitting ChunkLoad via bus"
        );
        batch_bytes = batch_bytes.saturating_add(packet.len());
        held.write(ColumnHeld {
            player: wanted.player,
            dim: wanted.at.dim,
            column: column_pos,
            held: true,
        });
        packet_writer.write(
            to(
                wanted.session,
                PacketPayload::ChunkLoad {
                    column: column_pos,
                    packet,
                },
            )
            .critical(),
        );
        sends = sends.saturating_add(1);
    }
    let Some(last) = batch.last() else {
        return;
    };
    if sends == 0 {
        return;
    }
    packet_writer.write(
        to(
            last.session,
            PacketPayload::ChunkBatchFinished(ClientboundChunkBatchFinished {
                batch_size: VarInt(sends),
            }),
        )
        .critical(),
    );
    chunk_view.unacknowledged_batches = chunk_view.unacknowledged_batches.saturating_add(1);
    chunk_view.batch_quota -= sends as f32;
}

/// Picks the next batch of the ready columns of a player into `picked`, nearest first.
fn pick_batch(
    (player, chunk_view, sent_view, in_dim, owner): (
        Entity,
        &ColumnView,
        &SentChunkView,
        &InDimension,
        &Owner,
    ),
    nearest: &mut Vec<ColumnPos>,
    picked: &mut Vec<WantedColumn>,
) {
    if !chunk_view.can_send() {
        return;
    }
    let allowed = chunk_view.batch_quota as usize;

    // Nearest to where the player is now, not to where they were when the column came
    // ready: one step of the player invalidates any order settled earlier. The ready
    // set is bounded by the view, so taking the batch out of it costs one pass and a
    // partial selection rather than a kept ordering that goes stale on its own.
    let center = sent_view
        .0
        .map_or(SectionPos::new(0, 0, 0), |view| view.center);
    let dim = in_dim.entity();
    nearest.clear();
    nearest.extend(chunk_view.sendable());
    if allowed < nearest.len() {
        nearest.select_nth_unstable_by_key(allowed, |pos| column_distance_sq(*pos, center));
        nearest.truncate(allowed);
    }
    nearest.sort_unstable_by_key(|pos| column_distance_sq(*pos, center));
    picked.extend(nearest.drain(..).map(|column| WantedColumn {
        player,
        session: owner.0,
        at: DimColumn { dim, column },
        source: match chunk_view.state(column) {
            Some(ColumnState::Stored) => PacketSource::DataLayer,
            _ => PacketSource::Dimension,
        },
    }));
}

/// Encodes every pending column, spread over the compute pool when there is more than one.
fn encode_shared(source: &ColumnSource, mut columns: &mut [SharedColumn]) {
    let direct_bits = DirectBits::of(&source.block_definitions, &source.registries);
    let encode_all = |_, columns: &mut [SharedColumn]| {
        for shared in columns {
            shared.encoding = match encode_column(source, direct_bits, shared.at) {
                Ok(packet) => ColumnEncoding::Encoded(packet),
                Err(skip) => ColumnEncoding::Skipped(skip),
            };
        }
    };
    let pool = ComputeTaskPool::try_get();
    let threads = pool.map_or(1, |pool| pool.thread_num()).max(1);
    let Some(pool) = pool.filter(|_| columns.len() > 1 && threads > 1) else {
        encode_all(0, columns);
        return;
    };
    // Columns differ in cost, so each thread gets several small chunks rather than one share.
    let chunk = columns.len().div_ceil(threads * 4).max(1);
    columns.par_chunk_map_mut(pool, chunk, encode_all);
}

enum ColumnSkip {
    /// A section the view holds a ticket on is not loaded.
    LostSection,
    /// A section did not encode; it is logged.
    Failed,
}

/// Encodes the whole `level_chunk_with_light` packet with its id.
fn encode_column(
    source: &ColumnSource,
    direct_bits: DirectBits,
    at: DimColumn,
) -> Result<Bytes, ColumnSkip> {
    let (Ok(chunk_index), Ok(column_index), Ok(type_config)) = (
        source.dim_chunk_indexes.get(at.dim),
        source.dim_column_indexes.get(at.dim),
        source.dim_type_configs.get(at.dim),
    ) else {
        return Err(ColumnSkip::Failed);
    };
    let column_pos = at.column;
    // The loading tickets this view holds keep every section of a pending column
    // loaded, so losing one here means the ticket and the send disagree about
    // what the view owns. Send nothing rather than a column the client can never
    // be sent again to fix.
    let held_sections = resolve_column(
        chunk_index,
        column_index,
        column_pos,
        type_config.section_count as i32,
        offset_sections(type_config.min_y),
    )
    .map(|(column_entity, sections)| {
        sections
            .into_iter()
            .map(|chunk_e| match source.stages.get(chunk_e) {
                Ok(SectionStage::Loaded) => source.chunks.get(chunk_e).ok(),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(|sections| (column_entity, sections))
    });
    let Some(Some((column_entity, sections))) = held_sections else {
        warn!(
            col_x = column_pos.x,
            col_z = column_pos.z,
            "a pending column lost a section it holds a ticket on"
        );
        return Err(ColumnSkip::LostSection);
    };

    let mut data = Vec::with_capacity(16 * 1024);
    let mut block_entities: Vec<ChunkDataBlockEntity<'static>> = Vec::new();
    for (_, _, section_block_entities) in &sections {
        for held_entity in section_block_entities.iter().flat_map(|held| held.iter()) {
            if let Ok(BlockEntity(entry)) = source.block_entities_held.get(*held_entity) {
                push_block_entity(entry, &mut block_entities);
            }
        }
    }
    encode_sections(
        column_pos,
        sections
            .iter()
            .map(|(blocks, biomes, _)| (*blocks, *biomes)),
        direct_bits,
        &mut data,
    )
    .ok_or(ColumnSkip::Failed)?;
    let light_data = if source.light_gated.is_none() {
        build_fullbright_light_data(type_config.section_count as usize + 2)
    } else {
        build_full_light_data(column_entity, &source.codec_params)
    };
    let heightmaps = source
        .column_heightmaps
        .get(column_entity)
        .map(|(surface, motion, no_leaves)| client_heightmaps(surface, motion, no_leaves))
        .unwrap_or_default();
    encode_packet(
        column_pos,
        &data,
        ColumnParts {
            heightmaps,
            block_entities,
            light_data,
        },
    )
    .ok_or(ColumnSkip::Failed)
}

#[inline]
fn offset_sections(min_y: i32) -> i32 {
    -(min_y >> SectionPos::BITS)
}

/// A column and the eight around it. A column's own light is only final once the eight around
/// it hold blocks, and a view's loading tickets keep that ring loaded.
fn margin_of(col: ColumnPos) -> impl Iterator<Item = ColumnPos> {
    col.square(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::PacketTarget;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_minecraft_engine_world::world::dimension::HasSkyLight;
    use mcrs_minecraft_engine_world::world::storage::column::{ColumnSections, ColumnSlot};

    const SECTIONS: u32 = 2;

    struct Fixture {
        dim: Entity,
        player: Entity,
        sections: FxHashMap<ColumnPos, Vec<Entity>>,
    }

    /// A dimension holding `columns`, each with every section loaded, and one player looking at
    /// the origin. Sections carry no light yet.
    fn fixture(world: &mut World, columns: &[ColumnPos]) -> Fixture {
        let dim = world.spawn_empty().id();
        let mut chunk_index = SectionIndex::new();
        let mut column_index = ColumnIndex::default();
        let mut sections = FxHashMap::default();

        for &col in columns {
            let entities: Vec<Entity> = (0..SECTIONS as i32)
                .map(|y| {
                    let section = world
                        .spawn((
                            mcrs_minecraft_engine_world::palette::air_section(),
                            BiomePalette::default(),
                            SectionStage::Loaded,
                        ))
                        .id();
                    chunk_index.insert(SectionPos::new(col.x, y, col.z), section);
                    section
                })
                .collect();
            let column = world
                .spawn((
                    ColumnSections {
                        min_section_y: 0,
                        sections: entities.iter().copied().map(Some).collect(),
                    },
                    InDimension(dim),
                ))
                .id();
            column_index.0.insert(
                EngineColumnPos::new(col.x, col.z),
                ColumnSlot {
                    entity: column,
                    section_count: SECTIONS,
                },
            );
            sections.insert(col, entities);
        }

        world.entity_mut(dim).insert((
            DimensionTypeConfig::new(0, SECTIONS << 4),
            HasSkyLight,
            ColumnLoading::bounded(0..=SECTIONS as i32 - 1),
            ColumnWaiters::default(),
            chunk_index,
            column_index,
        ));

        let player = world
            .spawn((
                ColumnView::default(),
                InDimension(dim),
                Owner(PlayerSession(1)),
            ))
            .id();

        world.insert_resource(LightGated);
        world.init_resource::<Messages<ColumnAwaitsLight>>();
        world.init_resource::<Messages<ColumnLightSettled>>();
        world.init_resource::<Messages<OutboundPlayerPacket>>();
        world.init_resource::<Messages<ColumnHeld>>();
        world.insert_resource(CurrentRegistries::new(
            mcrs_minecraft_game_registries::registry_load::registries_declaring(Vec::new()),
        ));
        world.insert_resource(mcrs_minecraft_worldgen_generator::tests::blocks().clone());

        Fixture {
            dim,
            player,
            sections,
        }
    }

    fn light(world: &mut World, sections: &[Entity]) {
        for &section in sections {
            world
                .entity_mut(section)
                .insert((BlockLight::default(), SkyLight::default()));
        }
    }

    fn sent_columns(world: &mut World) -> Vec<ColumnPos> {
        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .filter_map(|packet| match packet.data {
                PacketPayload::ChunkLoad { column, .. } => Some(column),
                _ => None,
            })
            .collect()
    }

    fn held_changes(world: &World) -> Vec<ColumnHeld> {
        let messages = world.resource::<Messages<ColumnHeld>>();
        messages.get_cursor().read(messages).copied().collect()
    }

    fn view(world: &World, player: Entity) -> &ColumnView {
        world.get::<ColumnView>(player).unwrap()
    }

    fn set_state(world: &mut World, player: Entity, col: ColumnPos, state: ColumnState) {
        let dim = world.get::<InDimension>(player).unwrap().entity();
        let [mut player_entity, mut dim_entity] = world.entity_mut([player, dim]);
        let mut waiters = dim_entity.get_mut::<ColumnWaiters>().unwrap();
        player_entity
            .get_mut::<ColumnView>()
            .unwrap()
            .set(&mut waiters, player, col, Some(state));
    }

    /// The whole point of holding the ready columns unordered: the batch is chosen against
    /// where the player is when it goes out, so the nearest column always leads it however the
    /// columns happened to arrive.
    fn a_batch_goes_out_nearest_first() {
        let mut world = World::new();
        let far = ColumnPos::new(5, 0);
        let near = ColumnPos::new(1, 0);
        let middle = ColumnPos::new(0, 3);
        let fx = fixture(&mut world, &[far, near, middle]);
        for sections in fx.sections.values() {
            let sections = sections.clone();
            light(&mut world, &sections);
        }

        world
            .get_mut::<ColumnView>(fx.player)
            .unwrap()
            .desired_columns_per_tick = 3.0;
        for col in [far, near, middle] {
            set_state(&mut world, fx.player, col, ColumnState::Ready);
        }

        world
            .run_system_once(send_column_queue)
            .expect("the send runs");

        assert_eq!(sent_columns(&mut world), vec![near, middle, far]);
        assert!(
            [near, middle, far]
                .iter()
                .all(|col| view(&world, fx.player).holds(*col))
        );
        assert_eq!(
            held_changes(&world).len(),
            3,
            "every column that went out is announced to its observers"
        );
    }

    /// A column is sent once and never again, so one that goes out before its light has settled
    /// is permanently black on the client. Readiness, not the send, is where that is decided.
    fn a_column_waits_for_the_light_of_every_section_it_carries() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        // Readiness reads the whole neighbourhood, so the margin is lit up
        // front and only the column's own sections are left to arrive.
        let neighbourhood: Vec<ColumnPos> = margin_of(col).collect();
        let fx = fixture(&mut world, &neighbourhood);
        for &neighbour in &neighbourhood {
            if neighbour != col {
                let lit = fx.sections[&neighbour].clone();
                light(&mut world, &lit);
            }
        }
        let sections = fx.sections[&col].clone();

        set_state(&mut world, fx.player, col, ColumnState::Awaiting);

        for (i, &section) in sections.iter().enumerate() {
            world
                .run_system_once(project_ready_columns)
                .expect("the projection runs");
            assert!(
                view(&world, fx.player)
                    .in_state(ColumnState::Ready)
                    .next()
                    .is_none(),
                "{} of {} sections lit",
                i,
                sections.len()
            );
            light(&mut world, &[section]);
        }

        world
            .run_system_once(project_ready_columns)
            .expect("the projection runs");
        assert_eq!(
            view(&world, fx.player).state(col),
            Some(ColumnState::Lighting)
        );
        let awaiting = world.resource::<Messages<ColumnAwaitsLight>>();
        assert_eq!(
            awaiting
                .get_cursor()
                .read(awaiting)
                .copied()
                .collect::<Vec<_>>(),
            vec![ColumnAwaitsLight(col)],
            "the light gate is asked about the column once it has landed"
        );

        world.write_message(ColumnLightSettled(col));
        world
            .run_system_once(promote_lit_columns)
            .expect("the promotion runs");
        assert_eq!(view(&world, fx.player).state(col), Some(ColumnState::Ready));
    }

    fn raise(world: &mut World, player: Entity, col: ColumnPos) {
        world.get_mut::<SentChunkView>(player).unwrap().0 = Some(ChunkTrackingView::default());
        set_state(world, player, col, ColumnState::Queued);
        world
            .run_system_once(raise_queued_columns)
            .expect("the raise runs");
    }

    fn ticketed(world: &World, dim: Entity, col: ColumnPos) -> bool {
        world.get::<ColumnLoading>(dim).unwrap().has_ticket(col)
    }

    fn a_despawned_view_hands_back_its_loading_tickets() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let fx = fixture(&mut world, &[col]);
        world.add_observer(release_loading_tickets);
        raise(&mut world, fx.player, col);
        assert!(ticketed(&world, fx.dim, col));

        world.despawn(fx.player);

        assert!(!ticketed(&world, fx.dim, col));
    }

    /// The loading tickets make this unreachable, so the guard is what stops a broken
    /// invariant from putting half a column on the wire.
    fn a_column_missing_a_section_goes_back_to_waiting_instead_of_out() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let fx = fixture(&mut world, &[col]);
        let sections = fx.sections[&col].clone();
        light(&mut world, &sections);

        set_state(&mut world, fx.player, col, ColumnState::Ready);
        world
            .get_mut::<SectionIndex>(fx.dim)
            .unwrap()
            .remove(SectionPos::new(col.x, 1, col.z));

        world
            .run_system_once(send_column_queue)
            .expect("the send runs");

        assert!(sent_columns(&mut world).is_empty());
        let chunk_view = view(&world, fx.player);
        assert!(chunk_view.in_state(ColumnState::Ready).next().is_none());
        assert_eq!(chunk_view.state(col), Some(ColumnState::Awaiting));
    }

    fn one_player_wanting(
        world: &mut World,
        wants: &[ColumnPos],
        looking: ChunkTrackingView,
    ) -> Entity {
        let fx = fixture(world, &[]);
        world.get_mut::<SentChunkView>(fx.player).unwrap().0 = Some(looking);
        for &col in wants {
            set_state(world, fx.player, col, ColumnState::Queued);
        }
        fx.player
    }

    /// Raising a column costs a whole column's worth of spawns. Counting the budget in sections
    /// would let the view ask for more than `spawn_chunks` can hand out, and the ticket queue
    /// would grow every tick without ever draining. What the budget leaves for later is the
    /// part of the view furthest from the player.
    fn the_view_raises_no_more_columns_than_a_tick_can_spawn_and_the_nearest_first() {
        let mut world = World::new();
        let budget = MAX_SPAWNS_PER_TICK / SECTIONS as usize;
        let wants: Vec<ColumnPos> = (0..=budget as i32).map(|x| ColumnPos::new(x, 0)).collect();
        let player = one_player_wanting(
            &mut world,
            &wants,
            ChunkTrackingView::new(SectionPos::new(0, 0, 0), u8::MAX, 8),
        );

        world
            .run_system_once(raise_queued_columns)
            .expect("the raise runs");

        let chunk_view = view(&world, player);
        assert_eq!(chunk_view.in_state(ColumnState::Awaiting).count(), budget);
        assert_eq!(
            chunk_view.state(ColumnPos::new(budget as i32, 0)),
            Some(ColumnState::Queued),
            "the furthest column is the one left for the next tick"
        );
    }

    fn looking_player(world: &mut World) -> Fixture {
        let fx = fixture(world, &[]);
        world.entity_mut(fx.player).insert((
            Transform::default(),
            PlayerViewDistance {
                distance: 2,
                vert_distance: 2,
            },
        ));
        fx
    }

    /// Vanilla's view is a Chebyshev square, so the corner columns of the square are owed just
    /// like the ones straight ahead.
    fn a_view_queues_every_column_of_its_square_in_the_tick_it_is_applied() {
        let mut world = World::new();
        let fx = looking_player(&mut world);

        world.run_system_once(update_view).expect("the view runs");

        let chunk_view = view(&world, fx.player);
        // The view reaches one column past the distance the client asked for.
        for x in -3..=3 {
            for z in -3..=3 {
                assert_eq!(
                    chunk_view.state(ColumnPos::new(x, z)),
                    Some(ColumnState::Queued),
                    "column {x},{z}"
                );
            }
        }
        assert_eq!(chunk_view.state(ColumnPos::new(4, 0)), None);
    }

    fn a_column_the_view_leaves_is_forgotten_by_the_client_and_its_tickets_released() {
        let mut world = World::new();
        let fx = looking_player(&mut world);
        world.run_system_once(update_view).expect("the view runs");

        let leaving = ColumnPos::new(-3, 0);
        set_state(&mut world, fx.player, leaving, ColumnState::Sent);
        world
            .get_mut::<ColumnLoading>(fx.dim)
            .unwrap()
            .add_ticket(leaving);
        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .clear();

        world.get_mut::<Transform>(fx.player).unwrap().translation.x = 16.0;
        world.run_system_once(update_view).expect("the view runs");

        assert_eq!(view(&world, fx.player).state(leaving), None);
        assert!(!ticketed(&world, fx.dim, leaving));
        assert!(
            world
                .resource_mut::<Messages<OutboundPlayerPacket>>()
                .drain()
                .any(|packet| matches!(packet.data, PacketPayload::ChunkUnload(ClientboundForgetLevelChunk { x, z }) if ColumnPos::new(x, z) == leaving))
        );
        assert!(held_changes(&world).contains(&ColumnHeld {
            player: fx.player,
            dim: fx.dim,
            column: leaving,
            held: false,
        }));
    }

    /// Two players that take the same columns in one send receive one encoding of each, and
    /// each player's batch stays bracketed and whole.
    fn players_sharing_a_column_share_its_encoding_in_whole_batches() {
        let mut world = World::new();
        let columns = [ColumnPos::new(0, 0), ColumnPos::new(1, 0)];
        let fx = fixture(&mut world, &columns);
        for sections in fx.sections.values() {
            let sections = sections.clone();
            light(&mut world, &sections);
        }
        let second = world
            .spawn((
                ColumnView::default(),
                InDimension(fx.dim),
                Owner(PlayerSession(2)),
            ))
            .id();
        for player in [fx.player, second] {
            for col in columns {
                set_state(&mut world, player, col, ColumnState::Ready);
            }
        }

        world
            .run_system_once(send_column_queue)
            .expect("the send runs");

        let packets: Vec<OutboundPlayerPacket> = world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect();
        let mut sessions: Vec<PlayerSession> = Vec::new();
        let mut loads: FxHashMap<ColumnPos, Vec<Bytes>> = FxHashMap::default();
        let mut sequence = Vec::new();
        for packet in &packets {
            let PacketTarget::SinglePlayer(session) = packet.target else {
                panic!("a column goes to one player");
            };
            if sessions.last() != Some(&session) {
                assert!(
                    !sessions.contains(&session),
                    "a player's batch is not split"
                );
                sessions.push(session);
            }
            sequence.push(match &packet.data {
                PacketPayload::ChunkBatchStart(_) => "start",
                PacketPayload::ChunkLoad { column, packet } => {
                    loads.entry(*column).or_default().push(packet.clone());
                    "load"
                }
                PacketPayload::ChunkBatchFinished(_) => "finish",
                other => panic!("unexpected {other:?}"),
            });
        }
        assert_eq!(sessions.len(), 2);
        let batch = ["start", "load", "load", "finish"];
        assert_eq!(sequence, [batch, batch].concat());
        for col in columns {
            let sent = &loads[&col];
            assert_eq!(sent.len(), 2);
            assert_eq!(
                sent[0].as_ptr(),
                sent[1].as_ptr(),
                "both players receive the one encoding of the column"
            );
        }
    }

    /// Light lands at the start of a tick, so the column it settles goes out in that tick.
    fn a_column_lit_at_the_start_of_a_tick_goes_out_in_that_tick() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let fx = fixture(&mut world, &[col]);
        let sections = fx.sections[&col].clone();
        light(&mut world, &sections);
        set_state(&mut world, fx.player, col, ColumnState::Lighting);

        world.write_message(ColumnLightSettled(col));
        world
            .run_system_once(promote_lit_columns)
            .expect("the promotion runs");
        world
            .run_system_once(send_column_queue)
            .expect("the send runs");

        assert_eq!(sent_columns(&mut world), vec![col]);
        assert!(view(&world, fx.player).holds(col));
    }

    fn spawn_player(world: &mut World, dim: Entity) -> Entity {
        // Any entity index of the world tells the players of a test apart.
        let session = PlayerSession(u64::from(world.spawn_empty().id().index_u32()));
        world
            .spawn((ColumnView::default(), InDimension(dim), Owner(session)))
            .id()
    }

    /// A player that leaves, or whose entity goes with a dimension change, must drop out of the
    /// waiters of its old dimension, or the index keeps routing columns to a view that is gone.
    fn a_view_that_goes_away_stops_waiting_on_its_columns() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let other = ColumnPos::new(1, 0);
        let fx = fixture(&mut world, &[col, other]);
        world.add_observer(release_loading_tickets);
        let staying = spawn_player(&mut world, fx.dim);
        let despawned = spawn_player(&mut world, fx.dim);
        for player in [fx.player, staying, despawned] {
            set_state(&mut world, player, col, ColumnState::Lighting);
        }
        set_state(&mut world, despawned, other, ColumnState::Awaiting);

        world.entity_mut(fx.player).remove::<ColumnView>();
        world.despawn(despawned);
        world.write_message(ColumnLightSettled(col));
        world
            .run_system_once(promote_lit_columns)
            .expect("the promotion runs");

        let waiters = world.get::<ColumnWaiters>(fx.dim).unwrap();
        assert!(
            waiters.of(col).is_empty(),
            "the one left waiting was promoted"
        );
        assert!(
            waiters.of(other).is_empty(),
            "a despawned view waits on nothing"
        );
        assert_eq!(view(&world, staying).state(col), Some(ColumnState::Ready));
    }

    #[test]
    fn a_view_raises_lights_sends_and_releases_its_columns() {
        a_view_that_goes_away_stops_waiting_on_its_columns();
        a_batch_goes_out_nearest_first();
        a_column_lit_at_the_start_of_a_tick_goes_out_in_that_tick();
        players_sharing_a_column_share_its_encoding_in_whole_batches();
        a_column_waits_for_the_light_of_every_section_it_carries();
        a_despawned_view_hands_back_its_loading_tickets();
        a_column_missing_a_section_goes_back_to_waiting_instead_of_out();
        the_view_raises_no_more_columns_than_a_tick_can_spawn_and_the_nearest_first();
        a_view_queues_every_column_of_its_square_in_the_tick_it_is_applied();
        a_column_the_view_leaves_is_forgotten_by_the_client_and_its_tickets_released();
    }
}
