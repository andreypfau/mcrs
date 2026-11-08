//! A summoned pig reaches a viewer as one spawn bundle, then falls onto the floor in the moves
//! that the viewer decodes.

use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::bus::{PacketPayload, to};
use mcrs_minecraft_engine::muxer::bridge::dispatch_encode;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine_physics::{EntityRows, Mobs};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_network::{OutboundReceiver, ServerSideConnection};
use mcrs_minecraft_protocol::packets::table::Decoded;
use mcrs_minecraft_protocol::packets::table::game_clientbound::ClientboundGamePacket as BasePacket;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{PositionPath, VecDelta};
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::table::game_clientbound::{self, ClientboundGamePacket};
use mcrs_minecraft_protocol_game::text::TextContent;
use mcrs_minecraft_server::VanillaEntityPhysicsPlugin;

use crate::commands::{chats, load_stone_column};
use crate::host_app::{self, PlayServer};
use crate::mock_connection;

const OPERATOR: Uuid = Uuid::from_u128(1);

/// What a viewer decoded of one mob, in the order the packets came.
#[derive(Debug, PartialEq)]
enum Seen {
    Delimiter,
    Add {
        id: i32,
        kind: EntityType,
        uuid: Uuid,
        pos: DVec3,
    },
    Data {
        id: i32,
        health: Option<f32>,
    },
    Attributes {
        id: i32,
        speed: Option<f64>,
    },
    Move {
        id: i32,
    },
    Sync {
        id: i32,
        on_ground: bool,
    },
    Motion {
        id: i32,
    },
}

struct Viewer {
    world: World,
    connection: Entity,
    frames: OutboundReceiver,
    seen: Vec<Seen>,
    /// The position of each mob as the client applies its moves.
    positions: Vec<(i32, DVec3)>,
}

impl Viewer {
    fn new() -> Self {
        let mut world = World::new();
        world.init_resource::<BridgeTelemetry>();
        world.init_resource::<mcrs_minecraft_engine::muxer::bridge::PendingConnections>();
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
            seen: Vec::new(),
            positions: Vec::new(),
        }
    }

    fn receive(&mut self, payloads: Vec<PacketPayload>) {
        let mut queue = self
            .world
            .get_mut::<OutboundQueue>(self.connection)
            .unwrap();
        for data in payloads {
            if matches!(data, PacketPayload::Replication(_)) {
                queue.push(to(PlayerSession(0), data));
            }
        }
        self.world.run_system_cached(dispatch_encode).unwrap();
        while let Ok(frames) = self.frames.try_recv() {
            let mut rest: &[u8] = &frames;
            while !rest.is_empty() {
                let length = VarInt::decode(&mut rest).unwrap().0 as usize;
                let (mut frame, tail) = rest.split_at(length);
                rest = tail;
                let id = VarInt::decode(&mut frame).unwrap().0;
                match game_clientbound::decode(id, &mut frame).unwrap() {
                    Decoded::Packet(packet) => {
                        assert!(
                            frame.is_empty(),
                            "packet {id} has {} trailing bytes",
                            frame.len()
                        );
                        self.apply(packet);
                    }
                    _ => {}
                }
            }
        }
    }

    fn apply(&mut self, packet: ClientboundGamePacket<'_>) {
        let seen = match packet {
            ClientboundGamePacket::Base(BasePacket::ClientboundBundleDelimiter(_)) => {
                Seen::Delimiter
            }
            ClientboundGamePacket::ClientboundAddEntity(add) => {
                self.positions.push((add.id.0, add.pos));
                Seen::Add {
                    id: add.id.0,
                    kind: add.kind,
                    uuid: add.uuid,
                    pos: add.pos,
                }
            }
            ClientboundGamePacket::ClientboundSetEntityData(data) => Seen::Data {
                id: data.entity_id.0,
                health: data.metadata.0.iter().find_map(|entry| match entry.value {
                    MetaDataValue::Float(health) if entry.index == 9 => Some(health),
                    _ => None,
                }),
            },
            ClientboundGamePacket::ClientboundUpdateAttributes(update) => Seen::Attributes {
                id: update.entity_id.0,
                speed: update
                    .attributes
                    .iter()
                    .find(|attribute| attribute.attribute == Attribute::MovementSpeed)
                    .map(|attribute| attribute.base),
            },
            ClientboundGamePacket::ClientboundMoveEntityPos(step) => {
                self.step(step.entity_id.0, &step.delta);
                Seen::Move {
                    id: step.entity_id.0,
                }
            }
            ClientboundGamePacket::ClientboundEntityPositionSync(sync) => {
                let to = match &sync.position {
                    PositionPath::Linear(to) => *to,
                    PositionPath::Stepped { steps, .. } => steps.last().unwrap().position,
                };
                self.place(sync.entity_id.0, to);
                Seen::Sync {
                    id: sync.entity_id.0,
                    on_ground: sync.on_ground,
                }
            }
            ClientboundGamePacket::ClientboundSetEntityMotion(motion) => Seen::Motion {
                id: motion.entity_id.0,
            },
            _ => return,
        };
        self.seen.push(seen);
    }

    fn place(&mut self, id: i32, to: DVec3) {
        if let Some((_, at)) = self.positions.iter_mut().find(|(seen, _)| *seen == id) {
            *at = to;
        }
    }

    fn step(&mut self, id: i32, delta: &VecDelta) {
        let steps: Vec<[i16; 3]> = match delta {
            VecDelta::Linear(delta) => vec![*delta],
            VecDelta::Stepped { steps, .. } => steps.iter().map(|step| step.delta).collect(),
        };
        let grid = |base: f64, steps: i16| {
            if steps == 0 {
                base
            } else {
                ((base * 4096.0 + 0.5).floor() + f64::from(steps)) / 4096.0
            }
        };
        if let Some((_, at)) = self.positions.iter_mut().find(|(seen, _)| *seen == id) {
            for [x, y, z] in steps {
                *at = DVec3::new(grid(at.x, x), grid(at.y, y), grid(at.z, z));
            }
        }
    }
}

fn start() -> PlayServer {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(VanillaEntityPhysicsPlugin);
    PlayServer::start(app, &[(OPERATOR, 2)], 0)
}

fn pig_rows(server: &mut PlayServer) -> Vec<DVec3> {
    server
        .world()
        .resource_mut::<EntityRows>()
        .tiles_mut()
        .flat_map(|(_, rows)| {
            rows.rows_mut()
                .map(|row| row.state.body.position)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// TRK-SPN-001, TRK-SPN-002, TRK-DATA-006, TRK-SUM-004, TRK-SUM-006: the bundle of a summoned pig
/// is the add, its health and its speed between two delimiters, at the centre of the block;
/// the pig falls onto the stone, and the client ends where the row ends.
#[test]
fn trk_spn_001_a_summoned_pig_reaches_a_viewer_as_one_bundle_and_falls_onto_the_floor() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));

    let replies = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded("summon minecraft:pig 4 70 4"),
        },
    );
    let message = chats(&replies).pop().expect("the summon replies");
    let TextContent::Translate { translate, .. } = &message.content else {
        panic!("a translated reply: {message:?}");
    };
    assert_eq!(translate, "commands.summon.success");
    viewer.receive(replies);
    viewer.receive(server.ticks(operator, 60));

    let mobs = server.world().resource::<Mobs>().0.clone();
    assert_eq!(mobs.len(), 1, "one summon makes one mob");
    let (&row_id, mob) = mobs.iter().next().unwrap();
    let id = row_id.0 as i32;
    let uuid = Uuid::from_u128(mob.uuid);
    assert_eq!(
        viewer.seen.get(..5),
        Some(
            &[
                Seen::Delimiter,
                Seen::Add {
                    id,
                    kind: EntityType::Pig,
                    uuid,
                    pos: DVec3::new(4.5, 70.0, 4.5),
                },
                Seen::Data {
                    id,
                    health: Some(10.0)
                },
                Seen::Attributes {
                    id,
                    speed: Some(0.25)
                },
                Seen::Delimiter,
            ][..]
        ),
        "{:#?}",
        viewer.seen
    );
    assert!(
        viewer.seen.contains(&Seen::Sync {
            id,
            on_ground: true
        }),
        "the landing sends the absolute position with the ground flag"
    );
    let rows = pig_rows(&mut server);
    assert_eq!(
        rows,
        [DVec3::new(4.5, 64.0, 4.5)],
        "the pig lies on the stone"
    );
    assert_eq!(
        viewer.positions,
        [(id, rows[0])],
        "the client shows the row"
    );
}
