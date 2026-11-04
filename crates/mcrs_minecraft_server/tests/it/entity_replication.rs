//! A model of the vanilla client decodes the bytes that each viewer receives and checks what it
//! shows: the other players near it, at their positions, and never itself.

use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bridge::dispatch_encode;
use mcrs_minecraft_engine::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine::bus::{PacketPayload, to};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_network::{OutboundReceiver, ServerSideConnection};
use mcrs_minecraft_protocol::packets::table::Decoded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{PositionPath, VecDelta};
use mcrs_minecraft_protocol_game::table::game_clientbound::{self, ClientboundGamePacket};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::host_app::{self, PlayServer, sent_to};
use crate::mock_connection;

/// What a vanilla client knows of the entities of its world.
#[derive(Default)]
struct ClientModel {
    own_id: i32,
    profiles: FxHashSet<Uuid>,
    /// The position of each entity, by wire id.
    entities: FxHashMap<i32, DVec3>,
    uuids: FxHashMap<i32, Uuid>,
}

impl ClientModel {
    fn apply(&mut self, packet: ClientboundGamePacket<'_>) {
        match packet {
            ClientboundGamePacket::ClientboundPlayerInfoUpdate(update) => {
                if update.actions.add_player() {
                    self.profiles
                        .extend(update.entries.iter().map(|entry| entry.player_uuid));
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

    /// Applies a relative move as the client does: from its last position on the step grid.
    fn step(&mut self, id: VarInt, delta: &VecDelta) {
        let VecDelta::Linear([x, y, z]) = *delta else {
            panic!("a stepped path is not sent: {delta:?}");
        };
        if let Some(pos) = self.entities.get_mut(&id.0) {
            let grid =
                |base: f64, steps: i16| ((base * 4096.0).round() + f64::from(steps)) / 4096.0;
            *pos = DVec3::new(grid(pos.x, x), grid(pos.y, y), grid(pos.z, z));
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
        PacketPayload::PlayerInfoUpdate { .. }
            | PacketPayload::SpawnBundle(_)
            | PacketPayload::RemoveEntities(_)
            | PacketPayload::EntityPosSync(_)
            | PacketPayload::EntityMove(_)
            | PacketPayload::EntityMoveRotate(_)
            | PacketPayload::EntityRotate(_)
            | PacketPayload::HeadRotation(_)
    )
}

struct Scene {
    server: PlayServer,
    anchors: [Entity; 2],
    uuids: [Uuid; 2],
    viewers: [Viewer; 2],
}

impl Scene {
    fn two_players() -> Self {
        let mut server = PlayServer::start(host_app::make_host_app(), &[], 0);
        let uuids = [Uuid::new_v4(), Uuid::new_v4()];
        let dim = server.dim;
        let anchors = [
            server.admit(dim, uuids[0], "First"),
            server.admit(dim, uuids[1], "Second"),
        ];
        let viewers = anchors.map(|anchor| {
            let session = server.session_of(anchor).unwrap();
            Viewer::new(mcrs_minecraft_engine::network_id::player_network_id(
                session,
            ))
        });
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
        for (viewer, anchor) in self.viewers.iter_mut().zip(self.anchors) {
            let session = self.server.session_of(anchor).unwrap();
            viewer.receive(sent_to(session, packets.clone()));
        }
    }

    fn place_second(&mut self, translation: DVec3) {
        let player = self.server.player(self.anchors[1]);
        self.server
            .world()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = translation;
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
    let mut scene = Scene::two_players();
    let start = scene.first_sees_second_at().unwrap();

    for step in 1..=20 {
        let target = start + DVec3::new(2.3 * f64::from(step), 0.0, 0.0);
        scene.place_second(target);
        let seen = scene.first_sees_second_at().unwrap();
        assert!(
            seen.distance(target) < 1e-3,
            "step {step}: the client shows the walker at {seen}, the server has {target}"
        );
    }
    let jumped = start + DVec3::new(60.0, 0.0, 0.0);
    scene.place_second(jumped);
    assert!(
        scene.first_sees_second_at().unwrap().distance(jumped) < 1e-3,
        "a jump of more than 8 blocks lands where the server has the walker"
    );

    scene.place_second(start + DVec3::new(300.0, 0.0, 0.0));

    assert_eq!(scene.first_sees_second_at(), None);
}
