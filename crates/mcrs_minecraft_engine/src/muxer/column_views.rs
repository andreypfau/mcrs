//! The view of each player: the muxer subscribes to the version stream of each column in the
//! view, whichever dimension or data layer makes the column, and sends the columns to the
//! client nearest first, at the rate that the client answers with.

use std::mem;

use bevy_ecs::entity::Entity;
use mcrs_engine_muxer::{ChunkStream, StreamNeed, ViewerId};
use mcrs_minecraft_engine_world::entity::player::chunk_view::{ChunkTrackingView, VIEW_MARGIN};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::lifecycle::ticket::MAX_SPAWNS_PER_TICK;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockChangedAck, ClientboundChunkBatchFinished, ClientboundChunkBatchStart,
    ClientboundChunkCacheRadius, ClientboundForgetLevelChunk, ClientboundSetChunkCacheCenter,
};
use mcrs_voxel::ColumnPos;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::block_action_ack::BlockActionAck;
use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, to};
use crate::column_change::ColumnChange;
use crate::column_request::ColumnRequest;
use crate::column_version::ColumnVersion;
use crate::muxer::batch_pacing::BatchPacing;
use crate::player_view::PlayerView;

/// A ceiling on one batch, not the rate: the client's answer is what paces the load, and a cap
/// low enough to bind first silently overrides it — at a full render distance a column is some
/// seventy kilobytes on the wire, so half the socket's queue was barely a hundred of them. The
/// bridge hands a blob over in pieces and buffers what the writer cannot take, and only kills a
/// connection sixteen socket queues behind, so a batch this size leaves it several ticks of room.
const MAX_BATCH_BYTES: usize = 4 * mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET;

/// The views of the players and the version streams of the columns they hold or wait for.
#[derive(Default)]
pub struct ColumnViews {
    players: FxHashMap<PlayerSession, PlayerColumns>,
    feeds: FxHashMap<DimColumn, ChunkStream<ColumnChange>>,
    /// What the muxer asks of each dimension in its next tick.
    requests: FxHashMap<Entity, Vec<ColumnRequest>>,
    sessions: Vec<PlayerSession>,
    nearest: Vec<ColumnPos>,
    picked: Vec<Picked>,
}

/// What the muxer asks of the dimension `dim` in its next tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DimRequests {
    pub dim: Entity,
    pub requests: Vec<ColumnRequest>,
}

fn viewer(session: PlayerSession) -> ViewerId {
    ViewerId(session.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DimColumn {
    dim: Entity,
    column: ColumnPos,
}

/// What a player is owed, column by column. A column of the view has exactly one state.
///
/// No pending state is ordered. A batch takes the columns nearest the player's position at the
/// moment it goes out, so any order stored earlier could only be a stale one — the player turns,
/// and the front of the queue is behind them.
struct PlayerColumns {
    dim: Entity,
    view: ChunkTrackingView,
    states: FxHashMap<ColumnPos, ColumnState>,
    queued: FxHashSet<ColumnPos>,
    wanted: FxHashSet<ColumnPos>,
    pacing: BatchPacing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColumnState {
    /// In the view, not asked for yet.
    Queued,
    /// Waiting for a version of the column.
    Wanted,
    Sent,
}

impl PlayerColumns {
    fn new(dim: Entity, view: ChunkTrackingView) -> Self {
        Self {
            dim,
            view,
            states: FxHashMap::default(),
            queued: FxHashSet::default(),
            wanted: FxHashSet::default(),
            pacing: BatchPacing::default(),
        }
    }

    fn set(&mut self, column: ColumnPos, state: Option<ColumnState>) -> Option<ColumnState> {
        let previous = match state {
            Some(state) => self.states.insert(column, state),
            None => self.states.remove(&column),
        };
        match previous {
            Some(ColumnState::Queued) => {
                self.queued.remove(&column);
            }
            Some(ColumnState::Wanted) => {
                self.wanted.remove(&column);
            }
            Some(ColumnState::Sent) | None => {}
        }
        match state {
            Some(ColumnState::Queued) => {
                self.queued.insert(column);
            }
            Some(ColumnState::Wanted) => {
                self.wanted.insert(column);
            }
            Some(ColumnState::Sent) | None => {}
        }
        previous
    }

    fn center(&self) -> ColumnPos {
        ColumnPos::from(self.view.center)
    }

    /// The columns a tick may ask for: a column costs the dimension a whole column's worth of
    /// sections, so counting sections here would let a view ask for more than a tick can spawn.
    fn raise_budget(&self) -> usize {
        let rows = self
            .view
            .max_section_y
            .saturating_sub(self.view.min_section_y)
            .saturating_add(1);
        let rows = usize::try_from(rows).unwrap_or(1).max(1);
        (MAX_SPAWNS_PER_TICK / rows).max(1)
    }
}

/// A column of the next batch of a player.
struct Picked {
    session: PlayerSession,
    at: DimColumn,
}

impl ColumnViews {
    /// Whether the client of `session` holds `column`.
    #[cfg(any(test, feature = "world-access"))]
    #[must_use]
    pub fn holds(&self, session: PlayerSession, column: ColumnPos) -> bool {
        self.players
            .get(&session)
            .is_some_and(|player| player.states.get(&column) == Some(&ColumnState::Sent))
    }

    /// The view that the muxer serves the client of `session`.
    #[cfg(feature = "world-access")]
    #[must_use]
    pub fn view(&self, session: PlayerSession) -> Option<ChunkTrackingView> {
        self.players.get(&session).map(|player| player.view)
    }

    /// The client of `session` has taken a batch and says how many columns a tick it managed.
    pub fn acknowledge_batch(&mut self, session: PlayerSession, desired_columns_per_tick: f32) {
        if let Some(player) = self.players.get_mut(&session) {
            player.pacing.acknowledge(desired_columns_per_tick);
        }
    }

    fn request(&mut self, dim: Entity, request: ColumnRequest) {
        self.requests.entry(dim).or_default().push(request);
    }

    fn need(&mut self, at: DimColumn, need: StreamNeed) {
        match need {
            StreamNeed::Nothing => {}
            StreamNeed::Version => self.request(at.dim, ColumnRequest::Want(at.column)),
            StreamNeed::Drop => {
                self.feeds.remove(&at);
                self.request(at.dim, ColumnRequest::Drop(at.column));
            }
        }
    }

    /// Takes `session` off `column`, whatever its state.
    fn leave(&mut self, session: PlayerSession, at: DimColumn) {
        let Some(feed) = self.feeds.get_mut(&at) else {
            return;
        };
        let need = feed.leave(viewer(session));
        self.need(at, need);
    }

    /// Forgets the view of `session` without a word to its client, which drops what it holds
    /// on a respawn or a disconnect.
    fn close(&mut self, session: PlayerSession) {
        let Some(player) = self.players.remove(&session) else {
            return;
        };
        for (&column, state) in &player.states {
            if *state != ColumnState::Queued {
                self.leave(
                    session,
                    DimColumn {
                        dim: player.dim,
                        column,
                    },
                );
            }
        }
    }

    /// Lets go of everything the muxer holds for the dimension `dim`.
    pub fn close_dim(&mut self, dim: Entity) {
        self.players.retain(|_, player| player.dim != dim);
        self.feeds.retain(|at, _| at.dim != dim);
        self.requests.remove(&dim);
    }

    /// Applies a view that the dimension `dim` reported: tells the client where it is, queues
    /// the columns the view reveals, and lets go of the ones it leaves. The forget goes out from
    /// the same view diff that sent the column, as vanilla's chunk map does, so what the client
    /// holds is exactly what the muxer counts as sent.
    fn apply_view(
        &mut self,
        dim: Entity,
        reported: PlayerView,
        out: &mut Vec<OutboundPlayerPacket>,
    ) {
        let PlayerView { session, view } = reported;
        let current = self.players.get(&session).map(|player| player.dim);
        let Some(view) = view else {
            if current == Some(dim) {
                self.close(session);
            }
            return;
        };
        if current.is_some_and(|current| current != dim) {
            self.close(session);
        }
        let Some(player) = self.players.get_mut(&session) else {
            send_cache_view(None, view, session, out);
            let mut player = PlayerColumns::new(dim, view);
            view.each_column(|column| {
                player.set(column, Some(ColumnState::Queued));
            });
            self.players.insert(session, player);
            return;
        };
        let old = player.view;
        if old == view {
            return;
        }
        send_cache_view(Some(old), view, session, out);
        player.view = view;
        let mut left = Vec::new();
        ChunkTrackingView::diff_columns(
            &old,
            &view,
            |column| {
                if !player.states.contains_key(&column) {
                    player.set(column, Some(ColumnState::Queued));
                }
            },
            |column| left.push(column),
        );
        for column in left {
            let Some(player) = self.players.get_mut(&session) else {
                return;
            };
            let previous = player.set(column, None);
            if previous == Some(ColumnState::Sent) {
                out.push(
                    to(
                        session,
                        PacketPayload::ChunkUnload(ClientboundForgetLevelChunk {
                            x: column.x,
                            z: column.z,
                        }),
                    )
                    .critical(),
                );
            }
            if previous.is_some_and(|state| state != ColumnState::Queued) {
                self.leave(session, DimColumn { dim, column });
            }
        }
    }

    fn take_version(&mut self, dim: Entity, version: ColumnVersion) {
        let at = DimColumn {
            dim,
            column: version.column,
        };
        if let Some(feed) = self.feeds.get_mut(&at) {
            feed.publish(version.packet);
        }
    }

    /// Sends `change` to the clients that hold its column, and keeps it for the clients that
    /// wait for the version before it.
    fn take_change(
        &mut self,
        dim: Entity,
        change: ColumnChange,
        out: &mut Vec<OutboundPlayerPacket>,
    ) {
        let at = DimColumn {
            dim,
            column: change.column,
        };
        let Some(feed) = self.feeds.get_mut(&at) else {
            return;
        };
        // An event shows once, to the clients that hold the column when it happens.
        let need = match change.payload {
            PacketPayload::BlockEvent(_)
            | PacketPayload::LevelEvent(_)
            | PacketPayload::BlockDestruction(_) => StreamNeed::Nothing,
            _ => feed.log(&change),
        };
        let holders: SmallVec<[PlayerSession; 8]> = feed
            .holders()
            .iter()
            .map(|holder| PlayerSession(holder.0))
            .filter(|&holder| Some(holder) != change.except)
            .collect();
        if !holders.is_empty() {
            out.push(OutboundPlayerPacket {
                target: PacketTarget::PlayerSet(holders),
                priority: change.priority,
                data: change.payload,
                session: PlayerSession(0),
                epoch: 0,
            });
        }
        self.need(at, need);
    }

    /// Takes what the dimension `dim` published in one tick: the views of its players, then the
    /// versions of the columns, then the changes of the tick, which come after the versions.
    pub fn take_dim(
        &mut self,
        dim: Entity,
        published: DimColumns,
        out: &mut Vec<OutboundPlayerPacket>,
    ) {
        let DimColumns {
            views,
            versions,
            changes,
            acks,
        } = published;
        for view in views {
            self.apply_view(dim, view, out);
        }
        for version in versions {
            self.take_version(dim, version);
        }
        for change in changes {
            self.take_change(dim, change, out);
        }
        out.extend(acks.into_iter().map(|ack| {
            to(
                ack.session,
                PacketPayload::BlockChangedAck(ClientboundBlockChangedAck {
                    sequence: VarInt(ack.sequence),
                }),
            )
        }));
    }

    /// Asks the dimensions for the columns the views want, and sends each player that is not
    /// `blocked` the next batch of the columns whose version is in. Every player that takes a
    /// version gets the same bytes.
    pub fn send(
        &mut self,
        blocked: &FxHashSet<PlayerSession>,
        out: &mut Vec<OutboundPlayerPacket>,
    ) {
        let mut sessions = mem::take(&mut self.sessions);
        let mut nearest = mem::take(&mut self.nearest);
        let mut picked = mem::take(&mut self.picked);
        sessions.extend(
            self.players
                .keys()
                .copied()
                .filter(|session| !blocked.contains(session)),
        );
        sessions.sort_unstable_by_key(|session| session.0);
        // A player's raise touches only the columns of that player and the waiters of a feed,
        // never a version, so each player can raise and then pick in one pass.
        for &session in &sessions {
            self.raise_queued(session, &mut nearest);
            self.pick(session, &mut nearest, &mut picked);
        }
        self.send_batches(&picked, out);
        sessions.clear();
        picked.clear();
        self.sessions = sessions;
        self.nearest = nearest;
        self.picked = picked;
    }

    /// What the views ask of each dimension, since the last call.
    pub fn take_requests(&mut self) -> impl Iterator<Item = DimRequests> + '_ {
        self.requests
            .drain()
            .map(|(dim, requests)| DimRequests { dim, requests })
    }

    /// Asks for the queued columns of `session`, nearest first, up to the budget of a tick.
    fn raise_queued(&mut self, session: PlayerSession, nearest: &mut Vec<ColumnPos>) {
        let Some(player) = self.players.get_mut(&session) else {
            return;
        };
        nearest.extend(player.queued.iter().copied());
        keep_nearest(nearest, player.center(), player.raise_budget());
        let dim = player.dim;
        for column in nearest.iter().copied() {
            player.set(column, Some(ColumnState::Wanted));
        }
        for column in nearest.drain(..) {
            let at = DimColumn { dim, column };
            let need = self.feeds.entry(at).or_default().wait(viewer(session));
            self.need(at, need);
        }
    }

    /// Picks the next batch of `session` into `picked` when it has room for one, nearest first.
    fn pick(
        &mut self,
        session: PlayerSession,
        nearest: &mut Vec<ColumnPos>,
        picked: &mut Vec<Picked>,
    ) {
        let Some(player) = self.players.get_mut(&session) else {
            return;
        };
        player.pacing.refill();
        let allowed = player.pacing.allowed();
        if allowed == 0 {
            return;
        }
        let dim = player.dim;
        let feeds = &self.feeds;
        nearest.extend(player.wanted.iter().copied().filter(|column| {
            feeds
                .get(&DimColumn {
                    dim,
                    column: *column,
                })
                .is_some_and(|feed| feed.version().is_some())
        }));
        // Nearest to where the player is now, not to where they were when the column came
        // ready: one step of the player invalidates any order settled earlier.
        keep_nearest(nearest, player.center(), allowed);
        picked.extend(nearest.drain(..).map(|column| Picked {
            session,
            at: DimColumn { dim, column },
        }));
    }

    /// Sends every player its batch, bracketed by a start and a finish, each column followed by
    /// the changes after its version.
    fn send_batches(&mut self, picked: &[Picked], out: &mut Vec<OutboundPlayerPacket>) {
        for batch in picked.chunk_by(|left, right| left.session == right.session) {
            let Some(first) = batch.first() else {
                continue;
            };
            let session = first.session;
            let mut sends = 0u32;
            let mut batch_bytes = 0usize;
            for picked in batch {
                if batch_bytes >= MAX_BATCH_BYTES {
                    break;
                }
                let Some(feed) = self.feeds.get_mut(&picked.at) else {
                    continue;
                };
                let Some(version) = feed.version() else {
                    continue;
                };
                let packet = &version.bytes;
                if sends == 0 {
                    out.push(
                        to(
                            session,
                            PacketPayload::ChunkBatchStart(ClientboundChunkBatchStart),
                        )
                        .critical(),
                    );
                }
                batch_bytes = batch_bytes.saturating_add(packet.len());
                out.push(
                    to(
                        session,
                        PacketPayload::ChunkLoad {
                            column: picked.at.column,
                            packet: packet.clone(),
                        },
                    )
                    .critical(),
                );
                for change in &version.changes {
                    let mut packet = to(session, change.payload.clone());
                    packet.priority = change.priority;
                    out.push(packet);
                }
                feed.hold(viewer(session));
                if let Some(player) = self.players.get_mut(&session) {
                    player.set(picked.at.column, Some(ColumnState::Sent));
                }
                sends = sends.saturating_add(1);
            }
            if sends == 0 {
                continue;
            }
            out.push(
                to(
                    session,
                    PacketPayload::ChunkBatchFinished(ClientboundChunkBatchFinished {
                        batch_size: VarInt(i32::try_from(sends).unwrap_or(i32::MAX)),
                    }),
                )
                .critical(),
            );
            if let Some(player) = self.players.get_mut(&session) {
                player.pacing.sent_batch(sends);
            }
        }
    }
}

/// Keeps the `count` columns of `columns` nearest `center`, nearest first.
fn keep_nearest(columns: &mut Vec<ColumnPos>, center: ColumnPos, count: usize) {
    if count < columns.len() {
        columns.select_nth_unstable_by_key(count, |column| column.distance_squared(center));
        columns.truncate(count);
    }
    columns.sort_unstable_by_key(|column| column.distance_squared(center));
}

/// Cache center and radius, sent Critical: the client renders nothing without them. The radius is
/// the view distance of the login, without the ring that the view holds past it.
fn send_cache_view(
    old: Option<ChunkTrackingView>,
    new: ChunkTrackingView,
    session: PlayerSession,
    out: &mut Vec<OutboundPlayerPacket>,
) {
    if old.is_none_or(|old| old.center != new.center) {
        out.push(
            to(
                session,
                PacketPayload::SetChunkCacheCenter(ClientboundSetChunkCacheCenter {
                    x: VarInt(new.center.x),
                    z: VarInt(new.center.z),
                }),
            )
            .critical(),
        );
    }
    if old.is_none_or(|old| old.distance != new.distance) {
        out.push(
            to(
                session,
                PacketPayload::SetChunkCacheRadius(ClientboundChunkCacheRadius {
                    radius: VarInt(i32::from(new.distance.saturating_sub(VIEW_MARGIN))),
                }),
            )
            .critical(),
        );
    }
}

/// What a dimension published for the views in one tick.
#[derive(Clone, Debug, Default)]
pub struct DimColumns {
    pub views: Vec<PlayerView>,
    pub versions: Vec<ColumnVersion>,
    pub changes: Vec<ColumnChange>,
    pub acks: Vec<BlockActionAck>,
}

impl DimColumns {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
            && self.versions.is_empty()
            && self.changes.is_empty()
            && self.acks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World as EcsWorld;
    use bytes::Bytes;
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundBlockEvent, ClientboundBlockUpdate, ClientboundLevelEvent,
    };
    use mcrs_voxel::{BlockPos, SectionPos};

    use crate::bus::PacketPriority;

    fn dim() -> Entity {
        EcsWorld::new().spawn_empty().id()
    }

    /// A view of the 3 x 3 columns around column `x`, 0, one section high.
    fn view_at(x: i32) -> ChunkTrackingView {
        ChunkTrackingView::with_y_bounds(SectionPos::new(x, 0, 0), 1, 1, 0, 0)
    }

    fn look(
        views: &mut ColumnViews,
        dim: Entity,
        session: PlayerSession,
        x: i32,
    ) -> Vec<OutboundPlayerPacket> {
        let mut out = Vec::new();
        views.apply_view(
            dim,
            PlayerView {
                session,
                view: Some(view_at(x)),
            },
            &mut out,
        );
        out
    }

    fn publish(views: &mut ColumnViews, dim: Entity, column: ColumnPos, packet: &'static [u8]) {
        views.take_version(
            dim,
            ColumnVersion {
                column,
                packet: Bytes::from_static(packet),
            },
        );
    }

    fn block_change(column: ColumnPos) -> ColumnChange {
        ColumnChange {
            column,
            priority: PacketPriority::Normal,
            payload: PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                block_pos: BlockPos::new(column.x * 16, 0, column.z * 16),
                block_state_id: mcrs_minecraft_protocol_game::block::BlockStateNumber(1),
            }),
            except: None,
        }
    }

    fn send(views: &mut ColumnViews) -> Vec<OutboundPlayerPacket> {
        let mut out = Vec::new();
        views.send(&FxHashSet::default(), &mut out);
        out
    }

    fn wants(views: &ColumnViews, dim: Entity) -> Vec<ColumnRequest> {
        views.requests.get(&dim).cloned().unwrap_or_default()
    }

    #[test]
    fn a_change_published_with_a_version_reaches_a_waiting_client_after_the_column() {
        let dim = dim();
        let session = PlayerSession(1);
        let column = ColumnPos::new(0, 0);
        let mut views = ColumnViews::default();
        look(&mut views, dim, session, 0);
        send(&mut views);

        publish(&mut views, dim, column, b"column");
        let mut out = Vec::new();
        views.take_change(dim, block_change(column), &mut out);
        assert!(out.is_empty(), "nobody holds the column yet");

        let sent: Vec<&str> = send(&mut views)
            .iter()
            .map(|packet| match &packet.data {
                PacketPayload::ChunkBatchStart(_) => "start",
                PacketPayload::ChunkLoad { .. } => "load",
                PacketPayload::BlockUpdate(_) => "change",
                PacketPayload::ChunkBatchFinished(_) => "finish",
                _ => "other",
            })
            .collect();
        assert_eq!(sent, ["start", "load", "change", "finish"]);

        views.take_change(dim, block_change(column), &mut out);
        assert!(
            matches!(&out[..], [packet] if matches!(&packet.target, PacketTarget::PlayerSet(set) if set.as_slice() == [session])),
            "a change after the send goes to the holder: {out:?}"
        );
    }

    #[test]
    fn a_block_event_reaches_only_the_clients_that_hold_the_column_when_it_happens() {
        let dim = dim();
        let session = PlayerSession(1);
        let column = ColumnPos::new(0, 0);
        let mut views = ColumnViews::default();
        look(&mut views, dim, session, 0);
        send(&mut views);
        let event = ColumnChange {
            column,
            priority: PacketPriority::Normal,
            payload: PacketPayload::BlockEvent(ClientboundBlockEvent {
                pos: BlockPos::new(0, 0, 0),
                action: 1,
                param: 0,
                block: VarInt(1),
            }),
            except: None,
        };

        publish(&mut views, dim, column, b"column");
        let mut out = Vec::new();
        views.take_change(dim, event.clone(), &mut out);
        assert!(out.is_empty(), "nobody holds the column yet");
        let sent = send(&mut views);
        assert!(
            !sent
                .iter()
                .any(|packet| matches!(packet.data, PacketPayload::BlockEvent(_))),
            "the event does not follow the column to a client that waited for it: {sent:?}"
        );

        views.take_change(dim, event, &mut out);
        assert!(
            matches!(&out[..], [packet] if matches!(packet.data, PacketPayload::BlockEvent(_))),
            "an event after the send goes to the holder: {out:?}"
        );
    }

    #[test]
    fn an_event_skips_the_client_that_caused_it_and_the_acks_follow_the_changes() {
        let dim = dim();
        let (breaker, watcher) = (PlayerSession(1), PlayerSession(2));
        let column = ColumnPos::new(0, 0);
        let mut views = ColumnViews::default();
        for session in [breaker, watcher] {
            look(&mut views, dim, session, 0);
        }
        send(&mut views);
        publish(&mut views, dim, column, b"column");
        send(&mut views);
        let event = ColumnChange {
            column,
            priority: PacketPriority::Normal,
            payload: PacketPayload::LevelEvent(ClientboundLevelEvent {
                effect: 2001,
                pos: BlockPos::new(0, 0, 0),
                effect_data: 1,
                global: false,
            }),
            except: Some(breaker),
        };

        let mut out = Vec::new();
        views.take_dim(
            dim,
            DimColumns {
                changes: vec![event, block_change(column)],
                acks: vec![BlockActionAck {
                    session: breaker,
                    sequence: 5,
                }],
                ..DimColumns::default()
            },
            &mut out,
        );

        let sent: Vec<(&str, Vec<PlayerSession>)> = out
            .iter()
            .map(|packet| {
                let to = match &packet.target {
                    PacketTarget::PlayerSet(set) => set.to_vec(),
                    PacketTarget::SinglePlayer(session) => vec![*session],
                    _ => Vec::new(),
                };
                let kind = match packet.data {
                    PacketPayload::LevelEvent(_) => "event",
                    PacketPayload::BlockUpdate(_) => "change",
                    PacketPayload::BlockChangedAck(_) => "ack",
                    _ => "other",
                };
                (kind, to)
            })
            .collect();
        assert_eq!(
            sent,
            [
                ("event", vec![watcher]),
                ("change", vec![breaker, watcher]),
                ("ack", vec![breaker]),
            ]
        );
    }

    #[test]
    fn clients_of_one_column_share_one_version_and_get_it_nearest_first() {
        let dim = dim();
        let near = ColumnPos::new(0, 0);
        let far = ColumnPos::new(1, 1);
        let mut views = ColumnViews::default();
        for session in [PlayerSession(1), PlayerSession(2)] {
            look(&mut views, dim, session, 0);
        }
        send(&mut views);
        let asked = wants(&views, dim);
        assert_eq!(
            asked
                .iter()
                .filter(|request| **request == ColumnRequest::Want(near))
                .count(),
            1,
            "one version is asked for all clients"
        );

        publish(&mut views, dim, far, b"far");
        publish(&mut views, dim, near, b"near");
        let loads: Vec<(PlayerSession, ColumnPos, Bytes)> = send(&mut views)
            .into_iter()
            .filter_map(|packet| match (packet.target, packet.data) {
                (
                    PacketTarget::SinglePlayer(session),
                    PacketPayload::ChunkLoad { column, packet },
                ) => Some((session, column, packet)),
                _ => None,
            })
            .collect();
        let order: Vec<(PlayerSession, ColumnPos)> = loads
            .iter()
            .map(|(session, column, _)| (*session, *column))
            .collect();
        assert_eq!(
            order,
            [
                (PlayerSession(1), near),
                (PlayerSession(1), far),
                (PlayerSession(2), near),
                (PlayerSession(2), far),
            ]
        );
        assert_eq!(
            loads[0].2.as_ptr(),
            loads[2].2.as_ptr(),
            "both clients get the same bytes of the column"
        );
    }

    #[test]
    fn a_column_that_leaves_the_view_is_forgotten_and_its_stream_dropped() {
        let dim = dim();
        let session = PlayerSession(1);
        let column = ColumnPos::new(-1, 0);
        let mut views = ColumnViews::default();
        look(&mut views, dim, session, 0);
        send(&mut views);
        publish(&mut views, dim, column, b"column");
        send(&mut views);
        assert!(views.holds(session, column));
        views.requests.clear();

        let out = look(&mut views, dim, session, 1);

        assert!(out.iter().any(|packet| matches!(
            packet.data,
            PacketPayload::ChunkUnload(ClientboundForgetLevelChunk { x: -1, z: 0 })
        )));
        assert!(wants(&views, dim).contains(&ColumnRequest::Drop(column)));
        assert!(!views.holds(session, column));
    }

    #[test]
    fn a_view_that_moves_to_another_dimension_lets_go_of_the_old_streams() {
        let mut world = EcsWorld::new();
        let (old, new) = (world.spawn_empty().id(), world.spawn_empty().id());
        let session = PlayerSession(1);
        let column = ColumnPos::new(0, 0);
        let mut views = ColumnViews::default();
        look(&mut views, old, session, 0);
        send(&mut views);
        publish(&mut views, old, column, b"column");
        send(&mut views);
        views.requests.clear();

        look(&mut views, new, session, 0);
        send(&mut views);

        assert!(wants(&views, old).contains(&ColumnRequest::Drop(column)));
        assert!(wants(&views, new).contains(&ColumnRequest::Want(column)));
        assert!(
            !views.holds(session, column),
            "the respawn drops what the client held"
        );
    }
}
