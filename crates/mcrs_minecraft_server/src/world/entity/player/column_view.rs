use crate::world::bus::to;
use crate::world::light_codec::{
    LightCodecParams, build_full_light_data, build_fullbright_light_data,
};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::ResMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::{Add, Discard};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Added, Changed, Component, ContainsEntity, Local, On, Or, Query};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::entity::Despawned;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::chunk_view::{ChunkTrackingView, PlayerViewDistance};
use mcrs_minecraft_level::palette::{BiomePalette, ChunkBlocks, non_air_block_count};
use mcrs_minecraft_level::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_level::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_level::world::lifecycle::ticket::{
    ChunkSpawnSet, MAX_SPAWNS_PER_TICK, SectionTickets, Ticket,
};
use mcrs_minecraft_level::world::lifecycle::trace as column_trace;
use mcrs_minecraft_level::world::lifecycle::trace::{ColumnStage, ColumnTraceLog};
use mcrs_minecraft_level::world::storage::block_entity::SectionBlockEntities;
use mcrs_minecraft_level::world::storage::column::{ColumnIndex, ColumnPos as EngineColumnPos};
use mcrs_minecraft_level::world::storage::section::SectionIndex;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::chunk::ChunkDataBlockEntity;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundChunkBatchFinished;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundChunkBatchStart;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundChunkCacheRadius;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundForgetLevelChunk;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSetChunkCacheCenter;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChunkBatchReceived;
use mcrs_minecraft_protocol::{ColumnPos, Encode};

use crate::world::aoi::ColumnHeld;
use crate::world::block_entity::{BlockEntity, packet_entry};
use crate::world::bus::{OutboundPlayerPacket, PacketPayload};
use crate::world::entity::player::HostAnchor;
use crate::world::heightmap::client_heightmaps;
use mcrs_minecraft_worldgen_generator::heightmap::{
    MotionHeightmap, NoLeavesHeightmap, SurfaceHeightmap,
};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{trace, warn};

pub struct ColumnViewPlugin;

/// Applies the view and turns what it wants into tickets, so the spawn that follows in the same
/// tick sees them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnViewSet;

impl Plugin for ColumnViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::Lighting>();
        app.configure_sets(FixedUpdate, (ColumnViewSet, ChunkSpawnSet).chain());
        app.add_systems(
            FixedUpdate,
            (
                update_view,
                raise_queued_columns,
                project_ready_columns,
                send_column_queue,
                crate::world::aoi::mirror_held_columns,
            )
                .chain()
                .in_set(ColumnViewSet),
        );

        app.add_observer(handle_batch_acknowledgement);
        app.add_observer(release_loading_tickets);
        app.add_observer(drop_view_of_departed_player);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColumnState {
    /// In the view, with no ticket raised for it yet.
    Queued,
    /// Ticketed, waiting for its sections to land and its light to settle.
    Awaiting,
    Ready,
    Sent,
}

impl ColumnState {
    fn unsent(self) -> Option<usize> {
        match self {
            Self::Queued => Some(0),
            Self::Awaiting => Some(1),
            Self::Ready => Some(2),
            Self::Sent => None,
        }
    }
}

/// What a player is owed, column by column, and the view that decided it. A column the view
/// wants has exactly one state, so this is the view's want list and no second copy of it is
/// kept.
///
/// No pending state is ordered. A batch takes the columns nearest the player's position at the
/// moment it goes out, so any order stored earlier could only be a stale one — the player turns,
/// and the front of the queue is behind them.
#[derive(Component)]
pub struct ColumnView {
    /// The view the client was last told about.
    view: Option<ChunkTrackingView>,
    states: FxHashMap<ColumnPos, ColumnState>,
    /// The unsent columns of `states` by state, so a drain walks what is left to do rather than
    /// the whole view. Kept by `set` alone.
    unsent: [FxHashSet<ColumnPos>; 3],
    batch_quota: f32,
    desired_columns_per_tick: f32,
    unacknowledged_batches: u32,
    max_unacknowledged_batches: u32,
}

impl Default for ColumnView {
    fn default() -> Self {
        Self {
            view: None,
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
        for column in columns {
            view.set(column, Some(ColumnState::Sent));
        }
        view
    }

    #[cfg(test)]
    pub(crate) fn looking_at(view: ChunkTrackingView) -> Self {
        Self {
            view: Some(view),
            ..Self::default()
        }
    }

    pub fn view(&self) -> Option<ChunkTrackingView> {
        self.view
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

    fn in_state(&self, state: ColumnState) -> impl Iterator<Item = ColumnPos> + '_ {
        state
            .unsent()
            .into_iter()
            .flat_map(|index| self.unsent[index].iter().copied())
    }

    fn set(&mut self, column: ColumnPos, state: Option<ColumnState>) -> Option<ColumnState> {
        let previous = match state {
            Some(state) => self.states.insert(column, state),
            None => self.states.remove(&column),
        };
        if let Some(index) = previous.and_then(ColumnState::unsent) {
            self.unsent[index].remove(&column);
        }
        if let Some(index) = state.and_then(ColumnState::unsent) {
            self.unsent[index].insert(column);
        }
        previous
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

fn handle_batch_acknowledgement(on: On<ReceivedPacketEvent>, mut players: Query<&mut ColumnView>) {
    let Some(ack) = on.decode::<ServerboundChunkBatchReceived>() else {
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
#[allow(clippy::type_complexity)]
pub(crate) fn update_view(
    mut players: Query<
        (
            Entity,
            &mut ColumnView,
            &Transform,
            &PlayerViewDistance,
            &InDimension,
            &HostAnchor,
        ),
        Or<(
            Changed<Transform>,
            Changed<PlayerViewDistance>,
            Added<ColumnView>,
        )>,
    >,
    mut dims: Query<(&mut SectionTickets, &DimensionTypeConfig)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut held: MessageWriter<ColumnHeld>,
    mut left: Local<Vec<ColumnPos>>,
    mut traces: Option<ResMut<ColumnTraceLog>>,
) {
    for (player, mut chunk_view, transform, distance, in_dim, host_anchor) in &mut players {
        let Ok((mut tickets, type_config)) = dims.get_mut(in_dim.entity()) else {
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
        let old_view = chunk_view.view;
        if old_view == Some(new_view) {
            continue;
        }
        send_cache_view(old_view, new_view, host_anchor.0, &mut packet_writer);

        let view = &mut *chunk_view;
        let mut queue = |column: ColumnPos| {
            if view.state(column).is_none() {
                view.set(column, Some(ColumnState::Queued));
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

        let off = offset_sections(type_config.min_y);
        for column in left.drain(..) {
            column_trace::forget(&mut traces, column);
            let Some(state) = view.set(column, None) else {
                continue;
            };
            if state == ColumnState::Queued {
                continue;
            }
            apply_loading_tickets(&mut tickets, column, off, type_config.section_count, false);
            if state == ColumnState::Sent {
                held.write(ColumnHeld {
                    player,
                    dim: in_dim.entity(),
                    column,
                    held: false,
                });
                packet_writer.write(
                    to(
                        host_anchor.0,
                        PacketPayload::ChunkUnload(ClientboundForgetLevelChunk {
                            x: column.x,
                            z: column.z,
                        }),
                    )
                    .critical(),
                );
            }
        }
        view.view = Some(new_view);
    }
}

/// Cache center and radius, sent Critical: the client renders nothing without them.
fn send_cache_view(
    old_view: Option<ChunkTrackingView>,
    new_view: ChunkTrackingView,
    host: Entity,
    packet_writer: &mut MessageWriter<OutboundPlayerPacket>,
) {
    if old_view.is_none_or(|old_view| old_view.center != new_view.center) {
        packet_writer.write(
            to(
                host,
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
                host,
                PacketPayload::SetChunkCacheRadius(ClientboundChunkCacheRadius {
                    radius: VarInt(new_view.distance as i32),
                }),
            )
            .critical(),
        );
    }
}

/// Raises what the view wants, nearest first, holding a loading ticket on every section of each
/// column it raises.
///
/// The budget is columns rather than sections: raising a column costs a whole column's worth of
/// spawns, so counting sections here would let the view ask for more than `spawn_chunks` can
/// ever hand out and grow the queue without bound.
pub(crate) fn raise_queued_columns(
    mut players: Query<(&mut ColumnView, &InDimension)>,
    mut dims: Query<(&mut SectionTickets, &DimensionTypeConfig)>,
    mut nearest: Local<Vec<ColumnPos>>,
    mut traces: Option<ResMut<ColumnTraceLog>>,
) {
    for (mut chunk_view, in_dim) in &mut players {
        let Some(view) = chunk_view.view else {
            continue;
        };
        nearest.extend(chunk_view.in_state(ColumnState::Queued));
        if nearest.is_empty() {
            continue;
        }
        let Ok((mut tickets, type_config)) = dims.get_mut(in_dim.entity()) else {
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

        let off = offset_sections(type_config.min_y);
        for column in nearest.drain(..) {
            chunk_view.set(column, Some(ColumnState::Awaiting));
            apply_loading_tickets(&mut tickets, column, off, type_config.section_count, true);
            column_trace::mark(&mut traces, column, ColumnStage::Ticketed);
        }
    }
}

/// Only the view knows which sections it ticketed, so a player leaving the
/// dimension that does not hand them back here pins them loaded for good.
fn release_loading_tickets(
    discard: On<Discard, ColumnView>,
    players: Query<(&ColumnView, &InDimension)>,
    mut dims: Query<(&mut SectionTickets, &DimensionTypeConfig)>,
) {
    let Ok((view, in_dim)) = players.get(discard.event().entity) else {
        return;
    };
    let Ok((mut tickets, type_config)) = dims.get_mut(in_dim.entity()) else {
        return;
    };
    let off = offset_sections(type_config.min_y);
    for (&column, state) in &view.states {
        if *state != ColumnState::Queued {
            apply_loading_tickets(&mut tickets, column, off, type_config.section_count, false);
        }
    }
}

/// A player whose move to another dimension was confirmed stays behind as a `Despawned`
/// entity, so its view goes here or it keeps its tickets for as long as the dimension runs.
fn drop_view_of_departed_player(add: On<Add, Despawned>, mut commands: Commands) {
    if let Ok(mut player) = commands.get_entity(add.event().entity) {
        player.try_remove::<ColumnView>();
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

/// Moves a column from "wanted" to "ready to send" once every section it carries has landed
/// and its light has settled. The only writer of that transition.
///
/// The light gate belongs here rather than at the send: a column goes out once and never
/// again, so sending one before its light has settled leaves it permanently black on the
/// client.
pub(crate) fn project_ready_columns(
    mut players: Query<(&mut ColumnView, &InDimension)>,
    dims: Query<(&SectionIndex, &ColumnIndex, &DimensionTypeConfig)>,
    chunks: Query<&SectionStage>,
    codec_params: LightCodecParams,
    light_status: mcrs_minecraft_light::prelude::LightStatus,
    lighting: Res<crate::Lighting>,
    mut ready: Local<Vec<ColumnPos>>,
    mut traces: Option<ResMut<ColumnTraceLog>>,
) {
    let await_light = light_status.is_installed() && *lighting == crate::Lighting::Propagated;
    for (mut chunk_view, dim) in &mut players {
        let Ok((chunk_index, column_index, type_config)) = dims.get(dim.entity()) else {
            continue;
        };
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
                !await_light
                    || sections.iter().all(|&chunk_e| {
                        codec_params.block_lights.contains(chunk_e)
                            && codec_params.sky_lights.contains(chunk_e)
                    })
            })
        };
        // Light computed against a neighbour whose blocks have not arrived
        // is a seam the column would carry for as long as the client holds
        // it, because a column goes out once. `settled_around` then waits
        // for the work those neighbours raised.
        ready.extend(chunk_view.in_state(ColumnState::Awaiting).filter(|&col| {
            light_landed(col)
                && margin_of(col).all(blocks_landed)
                && (!await_light || light_status.settled_around(col))
        }));
        for col in ready.drain(..) {
            trace!("Column {:?} ready", col);
            column_trace::mark(&mut traces, col, ColumnStage::Ready);
            chunk_view.set(col, Some(ColumnState::Ready));
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

/// Chunk-load wire emit routed through the `OutboundPlayerPacket` bus.
///
/// The rate is the client's: each batch is bracketed by a start and a finish, and the client
/// answers with the columns a tick it managed. Chunks are sent at Critical priority so they
/// are never dropped by the bridge.
#[allow(clippy::too_many_arguments)]
pub(crate) fn send_column_queue(
    mut players: Query<(Entity, &mut ColumnView, &InDimension, &HostAnchor)>,
    chunks: Query<(&ChunkBlocks, &BiomePalette, Option<&SectionBlockEntities>)>,
    stages: Query<&SectionStage>,
    block_entities_held: Query<&'static BlockEntity>,
    dim_chunk_indexes: Query<&SectionIndex>,
    dim_column_indexes: Query<&ColumnIndex>,
    dim_type_configs: Query<&DimensionTypeConfig>,
    column_heightmaps: Query<(&SurfaceHeightmap, &MotionHeightmap, &NoLeavesHeightmap)>,
    codec_params: LightCodecParams,
    lighting: Res<crate::Lighting>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut held: MessageWriter<ColumnHeld>,
    mut traces: Option<ResMut<ColumnTraceLog>>,
    mut nearest: Local<Vec<ColumnPos>>,
    mut batch: Local<Vec<PacketPayload>>,
) {
    players
        .iter_mut()
        .for_each(|(player, mut chunk_view, in_dim, host_anchor)| {
            let host = host_anchor.0;
            let Ok(chunk_index) = dim_chunk_indexes.get(in_dim.entity()) else {
                return;
            };
            let Ok(column_index) = dim_column_indexes.get(in_dim.entity()) else {
                return;
            };
            let Ok(type_config) = dim_type_configs.get(in_dim.entity()) else {
                return;
            };
            let wire_light_rows = type_config.section_count as usize + 2;
            let section_count = type_config.section_count as i32;
            let off = offset_sections(type_config.min_y);

            if chunk_view.unacknowledged_batches >= chunk_view.max_unacknowledged_batches {
                return;
            }
            let desired = chunk_view.desired_columns_per_tick;
            chunk_view.batch_quota = (chunk_view.batch_quota + desired).min(desired.max(1.0));
            if chunk_view.batch_quota < 1.0 {
                return;
            }
            let allowed = chunk_view.batch_quota as usize;
            if allowed == 0 || chunk_view.in_state(ColumnState::Ready).next().is_none() {
                return;
            }

            // Nearest to where the player is now, not to where they were when the column came
            // ready: one step of the player invalidates any order settled earlier. The ready
            // set is bounded by the view, so taking the batch out of it costs one pass and a
            // partial selection rather than a kept ordering that goes stale on its own.
            let center = chunk_view
                .view
                .map(|view| view.center)
                .unwrap_or(SectionPos::new(0, 0, 0));
            nearest.clear();
            nearest.extend(chunk_view.in_state(ColumnState::Ready));
            if allowed < nearest.len() {
                nearest.select_nth_unstable_by_key(allowed, |pos| column_distance_sq(*pos, center));
                nearest.truncate(allowed);
            }
            nearest.sort_unstable_by_key(|pos| column_distance_sq(*pos, center));

            let mut sends = 0usize;
            let mut batch_bytes = 0usize;
            batch.clear();

            for column_pos in nearest.drain(..) {
                if batch_bytes >= MAX_BATCH_BYTES {
                    break;
                }

                // The loading tickets this view holds keep every section of a pending column
                // loaded, so losing one here means the ticket and the send disagree about
                // what the view owns. Send nothing rather than a column the client can never
                // be sent again to fix.
                let held_sections =
                    resolve_column(chunk_index, column_index, column_pos, section_count, off).map(
                        |(column_entity, sections)| {
                            sections
                                .into_iter()
                                .map(|chunk_e| match stages.get(chunk_e) {
                                    Ok(SectionStage::Loaded) => chunks.get(chunk_e).ok(),
                                    _ => None,
                                })
                                .collect::<Option<Vec<_>>>()
                                .map(|sections| (column_entity, sections))
                        },
                    );
                let Some(Some((column_entity, sections))) = held_sections else {
                    warn!(
                        col_x = column_pos.x,
                        col_z = column_pos.z,
                        "a pending column lost a section it holds a ticket on"
                    );
                    chunk_view.set(column_pos, Some(ColumnState::Awaiting));
                    continue;
                };

                let mut data = Vec::with_capacity(16 * 1024);
                let mut block_entities: Vec<ChunkDataBlockEntity<'static>> = Vec::new();
                for (blocks, biomes, section_block_entities) in sections {
                    for held_entity in section_block_entities.iter().flat_map(|index| index.iter())
                    {
                        let Ok(BlockEntity(held_entity)) = block_entities_held.get(*held_entity)
                        else {
                            continue;
                        };
                        match packet_entry(held_entity) {
                            Ok(entry) => block_entities.push(entry),
                            Err(err) => warn!(%err, "encoding a block entity for the wire"),
                        }
                    }
                    // section and turns the rest of the column into garbage.
                    non_air_block_count(blocks)
                        .encode(&mut data)
                        .expect("Failed to encode chunk block count");
                    0u16.encode(&mut data)
                        .expect("Failed to encode chunk fluid count");
                    blocks
                        .0
                        .0
                        .encode(&mut data)
                        .expect("Failed to encode chunk block data");
                    biomes
                        .0
                        .encode(&mut data)
                        .expect("Failed to encode chunk biome data");
                }
                let light_data = if *lighting == crate::Lighting::FullSky {
                    build_fullbright_light_data(wire_light_rows)
                } else {
                    build_full_light_data(column_entity, &codec_params)
                };

                let heightmaps = column_heightmaps
                    .get(column_entity)
                    .map(|(surface, motion, no_leaves)| {
                        client_heightmaps(surface, motion, no_leaves)
                    })
                    .unwrap_or_default();

                column_trace::mark(&mut traces, column_pos, ColumnStage::Sent);
                chunk_view.set(column_pos, Some(ColumnState::Sent));
                held.write(ColumnHeld {
                    player,
                    dim: in_dim.entity(),
                    column: column_pos,
                    held: true,
                });

                trace!(
                    target: "mcrs_minecraft_server::player",
                    host_anchor = ?host,
                    col_x = column_pos.x,
                    col_z = column_pos.z,
                    bytes = data.len(),
                    "send_column_queue: emitting ChunkLoad via bus"
                );

                batch_bytes += data.len()
                    + (light_data.sky_light_arrays.len() + light_data.block_light_arrays.len())
                        * size_of::<mcrs_minecraft_protocol::chunk::LightChunk>();
                batch.push(PacketPayload::ChunkLoad {
                    column: column_pos,
                    chunk_bytes: data,
                    heightmaps,
                    light_data,
                    block_entities,
                });

                sends += 1;
            }
            if batch.is_empty() {
                return;
            }
            chunk_view.unacknowledged_batches += 1;
            chunk_view.batch_quota -= sends as f32;

            let batch_size = batch.len() as u32;
            let mut emit = |data| {
                packet_writer.write(to(host, data).critical());
            };
            emit(PacketPayload::ChunkBatchStart(ClientboundChunkBatchStart));
            for column in batch.drain(..) {
                emit(column);
            }
            emit(PacketPayload::ChunkBatchFinished(
                ClientboundChunkBatchFinished {
                    batch_size: VarInt(batch_size as i32),
                },
            ));
        })
}

#[inline]
fn offset_sections(min_y: i32) -> i32 {
    -(min_y >> SectionPos::BITS)
}

/// A column and the eight around it. A column's own light is only final once the eight around
/// it hold blocks, and the level of a view's loading tickets keeps that ring loaded.
fn margin_of(col: ColumnPos) -> impl Iterator<Item = ColumnPos> {
    (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| ColumnPos::new(col.x + dx, col.z + dz)))
}

fn apply_loading_tickets(
    tickets: &mut SectionTickets,
    col: ColumnPos,
    off_sections: i32,
    section_count: u32,
    add: bool,
) {
    for client_y in 0..section_count as i32 {
        let server_y = client_y - off_sections;
        let chunk_pos = SectionPos::new(col.x, server_y, col.z);
        if add {
            tickets.add(chunk_pos, Ticket::PLAYER_LOADING);
        } else {
            tickets.remove(chunk_pos, Ticket::PLAYER_LOADING);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_minecraft_level::world::dimension::HasSkyLight;
    use mcrs_minecraft_level::world::storage::column::{ColumnSections, ColumnSlot};
    use mcrs_minecraft_light::prelude::{
        BlockLight, LightBounds, LightProperties, LightRegistry, LightWorld, SkyLight,
        SpecialBlocks,
    };

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
                            ChunkBlocks::default(),
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
            SectionTickets::default(),
            chunk_index,
            column_index,
        ));

        let host = world.spawn_empty().id();
        let player = world
            .spawn((ColumnView::default(), InDimension(dim), HostAnchor(host)))
            .id();

        let registry = std::sync::Arc::new(LightRegistry::new(
            vec![LightProperties::AIR, LightProperties::SOLID],
            SpecialBlocks {
                unloaded: mcrs_minecraft_chunk::VoxelId(1),
                outside: mcrs_minecraft_chunk::VoxelId(0),
            },
        ));
        world.insert_resource(mcrs_minecraft_light::prelude::Lighting(LightWorld::new(
            registry,
            LightBounds::new(0, SECTIONS as i32 - 1),
        )));
        world.init_resource::<Messages<OutboundPlayerPacket>>();
        world.init_resource::<Messages<ColumnHeld>>();
        world.init_resource::<crate::Lighting>();

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
        world
            .get_mut::<ColumnView>(player)
            .unwrap()
            .set(col, Some(state));
    }

    /// The whole point of holding the ready columns unordered: the batch is chosen against
    /// where the player is when it goes out, so the nearest column always leads it however the
    /// columns happened to arrive.
    #[test]
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
    #[test]
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
        assert_eq!(view(&world, fx.player).state(col), Some(ColumnState::Ready));
    }

    fn raise(world: &mut World, player: Entity, col: ColumnPos) {
        {
            let mut chunk_view = world.get_mut::<ColumnView>(player).unwrap();
            chunk_view.view = Some(ChunkTrackingView::default());
            chunk_view.set(col, Some(ColumnState::Queued));
        }
        world
            .run_system_once(raise_queued_columns)
            .expect("the raise runs");
    }

    fn ticketed_sections(world: &World, dim: Entity, col: ColumnPos) -> usize {
        let tickets = world.get::<SectionTickets>(dim).unwrap();
        (0..SECTIONS as i32)
            .filter(|&y| {
                tickets
                    .loading_level(SectionPos::new(col.x, y, col.z))
                    .is_some()
            })
            .count()
    }

    #[test]
    fn a_despawned_view_hands_back_its_loading_tickets() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let fx = fixture(&mut world, &[col]);
        world.add_observer(release_loading_tickets);
        raise(&mut world, fx.player, col);
        assert_eq!(ticketed_sections(&world, fx.dim, col), SECTIONS as usize);

        world.despawn(fx.player);

        assert_eq!(ticketed_sections(&world, fx.dim, col), 0);
    }

    #[test]
    fn a_player_that_left_for_another_dimension_hands_back_its_loading_tickets() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let fx = fixture(&mut world, &[col]);
        world.add_observer(release_loading_tickets);
        world.add_observer(drop_view_of_departed_player);
        raise(&mut world, fx.player, col);

        world.entity_mut(fx.player).insert(Despawned);
        world.flush();

        assert_eq!(ticketed_sections(&world, fx.dim, col), 0);
    }

    /// The loading tickets make this unreachable, so the guard is what stops a broken
    /// invariant from putting half a column on the wire.
    #[test]
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
        let mut chunk_view = world.get_mut::<ColumnView>(fx.player).unwrap();
        chunk_view.view = Some(looking);
        for &col in wants {
            chunk_view.set(col, Some(ColumnState::Queued));
        }
        fx.player
    }

    #[test]
    fn a_wanted_column_is_raised_without_waiting_for_it_to_land() {
        let mut world = World::new();
        let wants = [
            ColumnPos::new(4, 0),
            ColumnPos::new(0, 0),
            ColumnPos::new(0, 2),
        ];
        let player = one_player_wanting(&mut world, &wants, ChunkTrackingView::default());

        world
            .run_system_once(raise_queued_columns)
            .expect("the raise runs");

        for col in wants {
            assert_eq!(view(&world, player).state(col), Some(ColumnState::Awaiting));
        }
    }

    /// Raising a column costs a whole column's worth of spawns. Counting the budget in sections
    /// would let the view ask for more than `spawn_chunks` can hand out, and the ticket queue
    /// would grow every tick without ever draining. What the budget leaves for later is the
    /// part of the view furthest from the player.
    #[test]
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
    #[test]
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

    #[test]
    fn a_column_the_view_leaves_is_forgotten_by_the_client_and_its_tickets_released() {
        let mut world = World::new();
        let fx = looking_player(&mut world);
        world.run_system_once(update_view).expect("the view runs");

        let leaving = ColumnPos::new(-3, 0);
        set_state(&mut world, fx.player, leaving, ColumnState::Sent);
        apply_loading_tickets(
            &mut world.get_mut::<SectionTickets>(fx.dim).unwrap(),
            leaving,
            0,
            SECTIONS,
            true,
        );
        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .clear();

        world.get_mut::<Transform>(fx.player).unwrap().translation.x = 16.0;
        world.run_system_once(update_view).expect("the view runs");

        assert_eq!(view(&world, fx.player).state(leaving), None);
        assert_eq!(ticketed_sections(&world, fx.dim, leaving), 0);
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
}
