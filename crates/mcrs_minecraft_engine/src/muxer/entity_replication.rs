//! The host side of entity replication: it hands the tile deltas of each dimension to the
//! muxer, and subscribes each player to the tiles around its own.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_engine_link::Link;
use mcrs_engine_muxer::{
    EntityEvent, EntityId, MuxerInput, SubjectEvent, TileDeltas, TileKey, View, ViewerId, WorldId,
};
use mcrs_minecraft_engine_world::session::{PlayerSession, SessionAnchors};
use mcrs_minecraft_entity_key::TILE_WIDTH;
use mcrs_minecraft_replication::{Backlog, EntityCodec, PlayerData, PlayerInput, ReplicationData};

use crate::muxer::latency::Latency;
use crate::muxer::muxer_link::MuxerLink;
use crate::muxer::muxer_stage::{MuxerCall, MuxerStage};
use crate::muxer::session::SessionConnection;
#[cfg(feature = "world-access")]
use crate::muxer::tile_delta_log::{LoggedDeltas, TileDeltaLog};
use crate::view_radius::VIEW_RADIUS;

/// The subscription of each player to the entity muxer of its world.
#[derive(Default)]
pub(crate) struct EntityReplication {
    worlds: BTreeSet<WorldId>,
    /// The player that each viewer subscribes with.
    viewers: BTreeMap<ViewerId, WorldPlayer>,
    /// The viewer of each player, by the world it is in.
    players: BTreeMap<WorldPlayer, ViewerId>,
    inputs: Vec<MuxerInput<EntityCodec>>,
    #[cfg(feature = "world-access")]
    pub(crate) published_tiles: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WorldPlayer {
    world: WorldId,
    subject: EntityId,
}

impl EntityReplication {
    /// Hands the tile deltas of one tick of `world` to the muxer, then subscribes each player
    /// that spawned or changed tile to the tiles around it, and forgets each player that left.
    fn publish(
        &mut self,
        link: &mut Link<MuxerStage>,
        world: WorldId,
        deltas: TileDeltas<ReplicationData>,
    ) {
        if self.worlds.insert(world) {
            link.send(MuxerCall::Entities(MuxerInput::Open {
                world,
                tile_width: TILE_WIDTH,
            }));
        }
        for delta in &deltas.tiles {
            for SubjectEvent { subject, event } in &delta.events {
                self.follow(world, delta.tile, *subject, event);
            }
        }
        #[cfg(feature = "world-access")]
        {
            self.published_tiles = self
                .published_tiles
                .saturating_add(deltas.tiles.len() as u64);
        }
        link.send(MuxerCall::Entities(MuxerInput::Publish { world, deltas }));
        for input in self.inputs.drain(..) {
            link.send(MuxerCall::Entities(input));
        }
    }

    fn follow(
        &mut self,
        world: WorldId,
        tile: TileKey,
        subject: EntityId,
        event: &EntityEvent<ReplicationData>,
    ) {
        let player = WorldPlayer { world, subject };
        match event {
            EntityEvent::Spawn(ReplicationData::Player(PlayerData::Spawn(spawn))) => {
                let viewer = spawn.viewer;
                if let Some(old) = self.viewers.get(&viewer)
                    && old.world != world
                {
                    self.inputs.push(MuxerInput::Unsubscribe {
                        world: old.world,
                        viewer,
                    });
                }
                self.viewers.insert(viewer, player);
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
                    .is_some_and(|watch| *watch == player)
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
    fn leave(&mut self, link: &mut Link<MuxerStage>, session: PlayerSession) {
        let viewer = ViewerId(session.0);
        if let Some(watch) = self.viewers.remove(&viewer) {
            link.send(MuxerCall::Entities(MuxerInput::Unsubscribe {
                world: watch.world,
                viewer,
            }));
        }
    }

    /// Hands `latency` of the player of `session` to the muxer of the world it watches.
    pub(crate) fn send_latency(
        &self,
        link: &mut Link<MuxerStage>,
        session: PlayerSession,
        latency: i32,
    ) {
        let viewer = ViewerId(session.0);
        self.send_input(link, viewer, PlayerInput::Latency { viewer, latency });
    }

    /// Hands the `backlog` of the connection of `session` to the muxer of the world it watches.
    pub(crate) fn send_backlog(
        &self,
        link: &mut Link<MuxerStage>,
        session: PlayerSession,
        backlog: Backlog,
    ) {
        let viewer = ViewerId(session.0);
        self.send_input(link, viewer, PlayerInput::Backlog { viewer, backlog });
    }

    fn send_input(&self, link: &mut Link<MuxerStage>, viewer: ViewerId, input: PlayerInput) {
        if let Some(watch) = self.viewers.get(&viewer) {
            link.send(MuxerCall::Entities(MuxerInput::Codec {
                world: watch.world,
                input,
            }));
        }
    }

    /// Stops the muxer of the dimension `label`.
    pub(crate) fn close(&mut self, link: &mut Link<MuxerStage>, label: Entity) {
        let world = world_of(label);
        if !self.worlds.remove(&world) {
            return;
        }
        self.viewers.retain(|_, watch| watch.world != world);
        self.players.retain(|player, _| player.world != world);
        link.send(MuxerCall::Entities(MuxerInput::Close { world }));
    }
}

/// The world of the dimension `label` in the muxer.
fn world_of(label: Entity) -> WorldId {
    WorldId(label.to_bits())
}

fn subscribe(
    world: WorldId,
    viewer: ViewerId,
    center: TileKey,
    subject: EntityId,
) -> MuxerInput<EntityCodec> {
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
pub fn publish(world: &mut World, label: Entity, deltas: TileDeltas<ReplicationData>) {
    #[cfg(feature = "world-access")]
    if let Some(mut log) = world.get_resource_mut::<TileDeltaLog>() {
        log.0.push(LoggedDeltas {
            dimension: label,
            deltas: deltas.clone(),
        });
    }
    let latencies: Vec<(PlayerSession, i32)> = deltas
        .tiles
        .iter()
        .flat_map(|delta| &delta.events)
        .filter_map(|event| match &event.event {
            EntityEvent::Spawn(ReplicationData::Player(PlayerData::Spawn(spawn))) => {
                let session = PlayerSession(spawn.viewer.0);
                Some((session, latency_of(world, session)?))
            }
            _ => None,
        })
        .collect();
    let mut muxer = world.get_resource_or_init::<MuxerLink>();
    let MuxerLink { link, entities, .. } = &mut *muxer;
    entities.publish(link, world_of(label), deltas);
    for (session, latency) in latencies {
        entities.send_latency(link, session, latency);
    }
}

/// Unsubscribes the player of `session` from the world it watches. The bytes the muxer owes it
/// there still come out, and none after them.
pub fn leave(world: &mut World, session: PlayerSession) {
    if let Some(mut muxer) = world.get_resource_mut::<MuxerLink>() {
        let MuxerLink { link, entities, .. } = &mut *muxer;
        entities.leave(link, session);
    }
}

fn latency_of(world: &World, session: PlayerSession) -> Option<i32> {
    let anchor = world.get_resource::<SessionAnchors>()?.get(session)?;
    let connection = world.get::<SessionConnection>(anchor)?.entity();
    world.get::<Latency>(connection).map(|latency| latency.0)
}
