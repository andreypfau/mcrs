//! The host side of entity replication: it hands the tile deltas of each dimension to the
//! muxer, subscribes each player to the tiles around its own, and appends the bytes of each
//! viewer to its connection.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use bevy_tasks::block_on;
use flume::Sender;
use mcrs_engine_link::{Link, Placement};
use mcrs_engine_muxer::{
    EntityEvent, EntityId, MuxerInput, MuxerSettings, MuxerStage, SubjectEvent, TileDeltas,
    TileKey, View, ViewerBytes, ViewerId, WorldId,
};
use mcrs_minecraft_engine_world::session::{PlayerSession, SessionAnchors};
use mcrs_minecraft_replication::{PlayerCodec, PlayerData, PlayerInput};

use crate::bus::{PacketPayload, to};
use crate::muxer::dim_outputs;
use crate::muxer::latency::Latency;
use crate::muxer::muxer_placement::MuxerPlacement;
use crate::muxer::session::SessionConnection;
use crate::muxer::tick_wake::TickWake;
use crate::muxer::tile_delta_log::{LoggedDeltas, TileDeltaLog};
use crate::player_tiles::{TILE_WIDTH, VIEW_RADIUS};

/// Whether [`deliver`] waits for the muxer to answer every input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answers {
    /// Takes the bytes that the muxer has handed out so far.
    Ready,
    /// Waits until the muxer owes nothing, then takes its bytes.
    All,
}

/// The link to the muxer and the subscription of each player.
#[derive(Resource)]
pub struct EntityReplication {
    link: Link<MuxerStage<PlayerCodec>>,
    worlds: BTreeSet<WorldId>,
    viewers: BTreeMap<ViewerId, Watch>,
    /// The viewer of each player, by the world it is in.
    players: BTreeMap<WorldPlayer, ViewerId>,
    inputs: Vec<MuxerInput<PlayerCodec>>,
    published_tiles: u64,
}

/// What a viewer subscribes to.
#[derive(Clone, Copy, Debug)]
struct Watch {
    world: WorldId,
    subject: EntityId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WorldPlayer {
    world: WorldId,
    subject: EntityId,
}

impl EntityReplication {
    /// A muxer on a thread of its own sends `wake` a message each time it hands out bytes.
    #[must_use]
    pub fn new(placement: Placement, wake: Sender<()>) -> Self {
        Self {
            link: Link::new_with_wake(MuxerStage::new(), placement, wake),
            worlds: BTreeSet::new(),
            viewers: BTreeMap::new(),
            players: BTreeMap::new(),
            inputs: Vec::new(),
            published_tiles: 0,
        }
    }

    /// The world of the dimension `label` in the muxer.
    #[must_use]
    pub fn world_of(label: Entity) -> WorldId {
        WorldId(label.to_bits())
    }

    /// The tile deltas handed to the muxer so far. The muxer encodes each one once.
    #[must_use]
    pub fn published_tiles(&self) -> u64 {
        self.published_tiles
    }

    /// The dimension whose tiles the player of `session` subscribes to.
    #[must_use]
    pub fn watched_world(&self, session: PlayerSession) -> Option<WorldId> {
        self.viewers
            .get(&ViewerId(session.0))
            .map(|watch| watch.world)
    }

    /// Hands the tile deltas of one tick of `world` to the muxer, then subscribes each player
    /// that spawned or changed tile to the tiles around it, and forgets each player that left.
    fn publish(&mut self, world: WorldId, deltas: TileDeltas<PlayerData>) {
        if self.worlds.insert(world) {
            self.link.send(MuxerInput::Open {
                world,
                codec: PlayerCodec::default(),
                settings: MuxerSettings {
                    tile_width: TILE_WIDTH,
                    ..MuxerSettings::default()
                },
            });
        }
        for delta in &deltas.tiles {
            for SubjectEvent { subject, event } in &delta.events {
                self.follow(world, delta.tile, *subject, event);
            }
        }
        self.published_tiles = self
            .published_tiles
            .saturating_add(deltas.tiles.len() as u64);
        self.link.send(MuxerInput::Publish { world, deltas });
        for input in self.inputs.drain(..) {
            self.link.send(input);
        }
    }

    fn follow(
        &mut self,
        world: WorldId,
        tile: TileKey,
        subject: EntityId,
        event: &EntityEvent<PlayerData>,
    ) {
        let player = WorldPlayer { world, subject };
        match event {
            EntityEvent::Spawn(PlayerData::Spawn(spawn)) => {
                let viewer = spawn.viewer;
                if let Some(old) = self.viewers.get(&viewer)
                    && old.world != world
                {
                    self.inputs.push(MuxerInput::Unsubscribe {
                        world: old.world,
                        viewer,
                    });
                }
                self.viewers.insert(viewer, Watch { world, subject });
                self.players.insert(player, viewer);
                self.inputs.push(subscribe(world, viewer, tile, subject));
            }
            EntityEvent::Enter { .. } => {
                if let Some(&viewer) = self.players.get(&player)
                    && self
                        .viewers
                        .get(&viewer)
                        .is_some_and(|watch| watch.world == world)
                {
                    self.inputs.push(subscribe(world, viewer, tile, subject));
                }
            }
            EntityEvent::Despawn => {
                let Some(viewer) = self.players.remove(&player) else {
                    return;
                };
                if self
                    .viewers
                    .get(&viewer)
                    .is_some_and(|watch| watch.world == world && watch.subject == subject)
                {
                    self.viewers.remove(&viewer);
                }
                self.inputs.push(MuxerInput::Unsubscribe { world, viewer });
            }
            _ => {}
        }
    }

    /// Unsubscribes the player of `session` from the world it watches. The bytes the muxer owes
    /// it there still come out, and none after them.
    pub fn leave(&mut self, session: PlayerSession) {
        let viewer = ViewerId(session.0);
        if let Some(watch) = self.viewers.remove(&viewer) {
            self.link.send(MuxerInput::Unsubscribe {
                world: watch.world,
                viewer,
            });
        }
    }

    /// Hands `latency` of the player of `session` to the muxer of the world it watches.
    pub fn send_latency(&mut self, session: PlayerSession, latency: i32) {
        let viewer = ViewerId(session.0);
        if let Some(watch) = self.viewers.get(&viewer) {
            self.link.send(MuxerInput::Codec {
                world: watch.world,
                input: PlayerInput::Latency { viewer, latency },
            });
        }
    }

    /// Stops the muxer of the dimension `label`.
    pub fn close(&mut self, label: Entity) {
        let world = Self::world_of(label);
        if !self.worlds.remove(&world) {
            return;
        }
        self.viewers.retain(|_, watch| watch.world != world);
        self.players.retain(|player, _| player.world != world);
        self.link.send(MuxerInput::Close { world });
    }

    fn take(&mut self, answers: Answers) -> Vec<ViewerBytes> {
        if answers == Answers::All {
            block_on(self.link.answered());
        }
        self.link.outputs().collect()
    }
}

fn subscribe(
    world: WorldId,
    viewer: ViewerId,
    center: TileKey,
    subject: EntityId,
) -> MuxerInput<PlayerCodec> {
    MuxerInput::Subscribe {
        world,
        viewer,
        view: View {
            center,
            radius: VIEW_RADIUS,
            subject: Some(subject),
        },
    }
}

/// Hands the tile deltas of the dimension `label` to the muxer. A player that comes into the
/// world gets the latency the host knows for it.
pub fn publish(world: &mut World, label: Entity, deltas: TileDeltas<PlayerData>) {
    if let Some(mut log) = world.get_resource_mut::<TileDeltaLog>() {
        log.0.push(LoggedDeltas {
            dimension: label,
            deltas: deltas.clone(),
        });
    }
    let spawned: Vec<PlayerSession> = deltas
        .tiles
        .iter()
        .flat_map(|delta| &delta.events)
        .filter_map(|event| match &event.event {
            EntityEvent::Spawn(PlayerData::Spawn(spawn)) => Some(PlayerSession(spawn.viewer.0)),
            _ => None,
        })
        .collect();
    let latencies: Vec<(PlayerSession, i32)> = spawned
        .into_iter()
        .filter_map(|session| Some((session, latency_of(world, session)?)))
        .collect();
    let mut replication = replication_mut(world);
    replication.publish(EntityReplication::world_of(label), deltas);
    for (session, latency) in latencies {
        replication.send_latency(session, latency);
    }
}

/// Unsubscribes the player of `session` from the world it watches, as
/// [`EntityReplication::leave`] does.
pub fn leave(world: &mut World, session: PlayerSession) {
    if let Some(mut replication) = world.get_resource_mut::<EntityReplication>() {
        replication.leave(session);
    }
}

/// Appends the bytes that the muxer handed out for each viewer to the outbound packets of its
/// session.
pub fn deliver(world: &mut World, answers: Answers) {
    let Some(mut replication) = world.get_resource_mut::<EntityReplication>() else {
        return;
    };
    let outputs = replication.take(answers);
    for ViewerBytes { viewer, chunks } in outputs {
        dim_outputs::deliver(
            world,
            to(PlayerSession(viewer.0), PacketPayload::Replication(chunks)),
        );
    }
}

fn replication_mut(world: &mut World) -> bevy_ecs::world::Mut<'_, EntityReplication> {
    if !world.contains_resource::<EntityReplication>() {
        let placement = world
            .get_resource::<MuxerPlacement>()
            .map_or(Placement::Inline, |placement| placement.0);
        let wake = world.get_resource_or_init::<TickWake>().waker();
        world.insert_resource(EntityReplication::new(placement, wake));
    }
    world.resource_mut::<EntityReplication>()
}

fn latency_of(world: &World, session: PlayerSession) -> Option<i32> {
    let anchor = world.get_resource::<SessionAnchors>()?.get(session)?;
    let connection = world.get::<SessionConnection>(anchor)?.entity();
    world.get::<Latency>(connection).map(|latency| latency.0)
}
