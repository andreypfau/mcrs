//! A model of the vanilla client decodes the bytes that each viewer receives and checks what it
//! shows: the other players near it, at their positions, and never itself, and at most
//! [`LISTED_CAP`] players in its player list.

use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine_link::Placement;
use mcrs_minecraft_engine::bus::{InboundPlayerDespawn, PacketPayload, to};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::muxer::bridge::dispatch_encode;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine::muxer::muxer_placement::MuxerPlacement;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_network::{OutboundReceiver, ServerSideConnection};
use mcrs_minecraft_protocol::packets::table::Decoded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{PositionPath, VecDelta};
use mcrs_minecraft_protocol_game::table::game_clientbound::{self, ClientboundGamePacket};
use mcrs_minecraft_replication::{LIST_INTERVAL_TICKS, LISTED_CAP};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::host_app::{self, PlayServer, sent_to};
use crate::mock_connection;

/// The ticks between two moves of a subject in the slowest tier.
const SLOWEST_TIER_TICKS: usize = 20;

/// The default cap of the muxer, K.
const CAP: usize = 200;

/// What a vanilla client knows of the entities of its world.
#[derive(Default)]
struct ClientModel {
    own_id: i32,
    profiles: FxHashSet<Uuid>,
    listed: FxHashSet<Uuid>,
    /// The position of each entity, by wire id.
    entities: FxHashMap<i32, DVec3>,
    uuids: FxHashMap<i32, Uuid>,
}

impl ClientModel {
    fn apply(&mut self, packet: ClientboundGamePacket<'_>) {
        match packet {
            ClientboundGamePacket::ClientboundPlayerInfoUpdate(update) => {
                for entry in update.entries.iter() {
                    let uuid = entry.player_uuid;
                    if update.actions.add_player() {
                        self.profiles.insert(uuid);
                    }
                    assert!(
                        self.profiles.contains(&uuid),
                        "an update for {uuid}, an entry that the client does not have"
                    );
                    if update.actions.update_listed() && entry.listed {
                        self.listed.insert(uuid);
                    } else if update.actions.update_listed() {
                        self.listed.remove(&uuid);
                    }
                }
                assert!(
                    self.listed.len() <= LISTED_CAP,
                    "the client lists {} players",
                    self.listed.len()
                );
            }
            ClientboundGamePacket::ClientboundPlayerInfoRemove(remove) => {
                for uuid in remove.players {
                    self.profiles.remove(&uuid);
                    self.listed.remove(&uuid);
                }
            }
            ClientboundGamePacket::ClientboundAddEntity(add) => {
                assert_ne!(add.id.0, self.own_id, "the viewer gets a spawn of itself");
                assert!(
                    self.profiles.contains(&add.uuid),
                    "the spawn of {} comes before its profile entry",
                    add.uuid
                );
                self.entities.insert(add.id.0, add.pos);
                self.uuids.insert(add.id.0, add.uuid);
            }
            ClientboundGamePacket::ClientboundRemoveEntities(remove) => {
                for id in remove.entity_ids {
                    self.entities.remove(&id.0);
                }
            }
            ClientboundGamePacket::ClientboundEntityPositionSync(sync) => {
                if let (Some(pos), PositionPath::Linear(to)) =
                    (self.entities.get_mut(&sync.entity_id.0), sync.position)
                {
                    *pos = to;
                }
            }
            ClientboundGamePacket::ClientboundMoveEntityPos(step) => {
                self.step(step.entity_id, &step.delta);
            }
            ClientboundGamePacket::ClientboundMoveEntityPosRot(step) => {
                self.step(step.entity_id, &step.delta);
            }
            _ => {}
        }
    }

    /// Applies a relative move as the client does: from its last position on the step grid,
    /// each step of a path after the one before.
    fn step(&mut self, id: VarInt, delta: &VecDelta) {
        let steps: Vec<[i16; 3]> = match delta {
            VecDelta::Linear(delta) => vec![*delta],
            VecDelta::Stepped(steps) => steps.iter().map(|step| step.delta).collect(),
        };
        if let Some(pos) = self.entities.get_mut(&id.0) {
            let grid =
                |base: f64, steps: i16| ((base * 4096.0).round() + f64::from(steps)) / 4096.0;
            for [x, y, z] in steps {
                *pos = DVec3::new(grid(pos.x, x), grid(pos.y, y), grid(pos.z, z));
            }
        }
    }

    fn position_of(&self, uuid: Uuid) -> Option<DVec3> {
        self.uuids
            .iter()
            .find(|(_, seen)| **seen == uuid)
            .and_then(|(id, _)| self.entities.get(id).copied())
    }
}

/// One viewer: the bytes its connection writes, framed, and the client model that reads them.
struct Viewer {
    world: World,
    connection: Entity,
    frames: OutboundReceiver,
    client: ClientModel,
    /// The framed bytes of the entity and player list packets that the viewer received.
    received_bytes: usize,
}

impl Viewer {
    fn new(own_id: i32) -> Self {
        let mut world = World::new();
        world.init_resource::<BridgeTelemetry>();
        let (raw, frames) = mock_connection::make_mock_raw_connection();
        let connection = world
            .spawn((
                ServerSideConnection { raw: Box::new(raw) },
                OutboundQueue::default(),
            ))
            .id();
        Self {
            world,
            connection,
            frames,
            client: ClientModel {
                own_id,
                ..ClientModel::default()
            },
            received_bytes: 0,
        }
    }

    /// Writes the entity packets of `payloads` through the bridge and feeds the decoded frames to
    /// the client model.
    fn receive(&mut self, payloads: Vec<PacketPayload>) {
        let mut queue = self
            .world
            .get_mut::<OutboundQueue>(self.connection)
            .unwrap();
        for data in payloads.into_iter().filter(is_entity_packet) {
            queue.push(to(PlayerSession(0), data));
        }
        self.world.run_system_cached(dispatch_encode).unwrap();
        while let Ok(frames) = self.frames.try_recv() {
            self.received_bytes += frames.len();
            let mut rest: &[u8] = &frames;
            while !rest.is_empty() {
                let length = VarInt::decode(&mut rest).unwrap().0 as usize;
                let (mut frame, tail) = rest.split_at(length);
                rest = tail;
                let id = VarInt::decode(&mut frame).unwrap().0;
                if let Decoded::Packet(packet) = game_clientbound::decode(id, &mut frame).unwrap() {
                    self.client.apply(packet);
                }
            }
        }
    }
}

fn is_entity_packet(data: &PacketPayload) -> bool {
    matches!(
        data,
        PacketPayload::PlayerInfoUpdate { .. } | PacketPayload::Replication(_)
    )
}

/// Players that join in one place; the first of them are viewers with a client model.
struct Scene {
    server: PlayServer,
    anchors: Vec<Entity>,
    uuids: Vec<Uuid>,
    viewers: Vec<Viewer>,
}

impl Scene {
    fn two_players() -> Self {
        Self::crowd(2, 2)
    }

    fn crowd(players: usize, viewers: usize) -> Self {
        Self::crowd_with(players, viewers, Placement::Inline)
    }

    fn crowd_with(players: usize, viewers: usize, muxer: Placement) -> Self {
        let mut app = host_app::make_host_app();
        app.insert_resource(MuxerPlacement(muxer));
        let mut server = PlayServer::start(app, &[], 0);
        let uuids: Vec<Uuid> = (0..players).map(|_| Uuid::new_v4()).collect();
        let dim = server.dim;
        let anchors: Vec<Entity> = uuids
            .iter()
            .enumerate()
            .map(|(index, &uuid)| server.admit(dim, uuid, &format!("Player{index}")))
            .collect();
        let viewers = anchors[..viewers]
            .iter()
            .map(|&anchor| {
                let session = server.session_of(anchor).unwrap();
                Viewer::new(mcrs_minecraft_engine::network_id::player_network_id(
                    session,
                ))
            })
            .collect();
        let mut scene = Self {
            server,
            anchors,
            uuids,
            viewers,
        };
        scene.tick(3);
        scene
    }

    fn tick(&mut self, n: usize) {
        let packets = self.server.ticks_all(n);
        for (viewer, &anchor) in self.viewers.iter_mut().zip(&self.anchors) {
            let Some(session) = self.server.session_of(anchor) else {
                continue;
            };
            viewer.receive(sent_to(session, packets.clone()));
        }
    }

    /// Takes the player of `anchors[index]` out as a disconnect does.
    fn disconnect(&mut self, index: usize) {
        let session = self.server.session_of(self.anchors[index]).unwrap();
        let dim = self.server.dim;
        self.server
            .submit_to(dim, ToDim::Despawn(InboundPlayerDespawn { session }));
    }

    fn place(&mut self, index: usize, translation: DVec3) {
        let player = self.server.player(self.anchors[index]);
        self.server
            .world()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = translation;
    }

    fn position(&mut self, index: usize) -> DVec3 {
        let player = self.server.player(self.anchors[index]);
        self.server
            .world()
            .get::<Transform>(player)
            .unwrap()
            .translation
    }

    fn place_second(&mut self, translation: DVec3) {
        self.place(1, translation);
        self.tick(1);
    }

    fn first_sees_second_at(&self) -> Option<DVec3> {
        self.viewers[0].client.position_of(self.uuids[1])
    }
}

#[test]
fn two_players_near_each_other_see_each_other_and_never_themselves() {
    let scene = Scene::two_players();

    assert!(scene.first_sees_second_at().is_some());
    assert!(
        scene.viewers[1]
            .client
            .position_of(scene.uuids[0])
            .is_some()
    );
}

#[test]
fn a_player_that_walks_away_moves_on_the_client_and_is_removed_out_of_range() {
    for muxer in [Placement::Inline, Placement::Thread] {
        walk_away(muxer);
    }
}

fn walk_away(muxer: Placement) {
    let mut scene = Scene::crowd_with(2, 2, muxer);
    let start = scene.first_sees_second_at().unwrap();

    for step in 1..=20 {
        let target = start + DVec3::new(2.3 * f64::from(step), 0.0, 0.0);
        scene.place_second(target);
        // The slowest tier sends a move each 20 ticks.
        scene.tick(SLOWEST_TIER_TICKS - 1);
        let seen = scene.first_sees_second_at().unwrap();
        assert!(
            seen.distance(target) < 1e-3,
            "{muxer:?}, step {step}: the client shows the walker at {seen}, the server has {target}"
        );
    }
    let jumped = start + DVec3::new(60.0, 0.0, 0.0);
    scene.place_second(jumped);
    scene.tick(SLOWEST_TIER_TICKS - 1);
    assert!(
        scene.first_sees_second_at().unwrap().distance(jumped) < 1e-3,
        "a jump of more than 8 blocks lands where the server has the walker"
    );

    scene.place_second(start + DVec3::new(300.0, 0.0, 0.0));

    assert_eq!(scene.first_sees_second_at(), None);
}

#[test]
fn a_crowd_lists_at_most_the_cap_and_costs_nothing_once_it_stands_still() {
    let mut scene = Scene::crowd(LISTED_CAP + 10, 1);
    scene.tick(LIST_INTERVAL_TICKS as usize);

    let client = &scene.viewers[0].client;
    assert_eq!(
        client.profiles.len(),
        LISTED_CAP + 10,
        "it has every player near it"
    );
    assert_eq!(client.listed.len(), LISTED_CAP);

    let settled = scene.viewers[0].received_bytes;
    scene.tick(2 * LIST_INTERVAL_TICKS as usize);
    assert_eq!(
        scene.viewers[0].received_bytes - settled,
        0,
        "players that neither move nor change send no entity or player list bytes"
    );
}

#[test]
fn a_player_that_disconnects_leaves_the_player_list_of_its_viewers() {
    let mut scene = Scene::two_players();
    assert!(scene.viewers[0].client.listed.contains(&scene.uuids[1]));

    scene.disconnect(1);
    scene.tick(2);

    assert!(!scene.viewers[0].client.profiles.contains(&scene.uuids[1]));
    assert_eq!(scene.first_sees_second_at(), None);
}

#[test]
fn a_crowd_past_the_cap_shows_at_most_the_cap_and_moves_each_player_it_shows() {
    let players = 320;
    let mut scene = Scene::crowd(players, 1);
    let origin = scene.position(0);
    // Seven tiles in a row around the viewer, about 46 players in each.
    for index in 1..players {
        let tile = (index % 7) as f64 - 3.0;
        scene.place(index, origin + DVec3::new(32.0 * tile, 0.0, 0.0));
    }
    scene.tick(SLOWEST_TIER_TICKS);
    let shown = scene.viewers[0].client.entities.len();
    assert!(
        (CAP / 2..=CAP).contains(&shown),
        "the client shows {shown} of {players} players"
    );

    let start: Vec<DVec3> = (0..players).map(|index| scene.position(index)).collect();
    for step in 1..=SLOWEST_TIER_TICKS {
        for (index, start) in start.iter().enumerate().skip(1) {
            scene.place(index, *start + DVec3::new(0.0, 0.0, 0.25 * step as f64));
        }
        scene.tick(1);
    }
    let client = &scene.viewers[0].client;
    let frozen = scene
        .uuids
        .iter()
        .zip(&start)
        .filter(|(uuid, start)| {
            client
                .position_of(**uuid)
                .is_some_and(|seen| seen.distance(**start) < 1e-3)
        })
        .count();
    assert_eq!(frozen, 0, "every player that the client shows moved");

    let exact = |scene: &mut Scene| {
        let at: Vec<DVec3> = (0..players).map(|index| scene.position(index)).collect();
        let client = &scene.viewers[0].client;
        scene
            .uuids
            .iter()
            .zip(&at)
            .filter(|(uuid, at)| {
                client
                    .position_of(**uuid)
                    .is_some_and(|seen| seen.distance(**at) < 1e-3)
            })
            .count()
    };
    scene.tick(2);
    assert!(
        exact(&mut scene) >= 31,
        "the nearest tier shows its players where they are within 2 ticks"
    );
    scene.tick(SLOWEST_TIER_TICKS);
    assert_eq!(
        exact(&mut scene),
        scene.viewers[0].client.entities.len(),
        "each tier shows its players where they stopped"
    );
}
