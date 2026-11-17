use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::{PacketPayload, to};
use mcrs_minecraft_engine::muxer::bridge::dispatch_encode;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_network::{OutboundReceiver, ServerSideConnection};
use mcrs_minecraft_protocol::packets::table::Decoded;
use mcrs_minecraft_protocol::packets::table::game_clientbound::ClientboundGamePacket as BasePacket;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{PositionPath, VecDelta};
use mcrs_minecraft_protocol_game::table::game_clientbound::{self, ClientboundGamePacket};
use mcrs_minecraft_server::{VanillaEntityPhysicsPlugin, VanillaFallingBlockPlugin};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_vanilla_falling_block::FallRequest;
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer};
use crate::mock_connection;

const VIEWER: Uuid = Uuid::from_u128(1);

#[derive(Debug, PartialEq)]
enum Path {
    Linear,
    Stepped,
}

#[derive(Debug, PartialEq)]
enum Seen {
    Delimiter,
    Add {
        id: i32,
        kind: EntityType,
        data: i32,
        position: DVec3,
        velocity: DVec3,
    },
    Motion {
        id: i32,
        velocity: DVec3,
    },
    Move {
        id: i32,
        path: Path,
    },
    Remove {
        ids: Vec<i32>,
    },
}

impl Seen {
    fn names(&self, entity: i32) -> bool {
        match self {
            Seen::Delimiter => false,
            Seen::Add { id, .. } | Seen::Motion { id, .. } | Seen::Move { id, .. } => *id == entity,
            Seen::Remove { ids } => ids.contains(&entity),
        }
    }
}

struct Viewer {
    world: World,
    connection: Entity,
    frames: OutboundReceiver,
    seen: Vec<Seen>,
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
                if let Decoded::Packet(packet) = game_clientbound::decode(id, &mut frame).unwrap() {
                    self.apply(packet);
                }
            }
        }
    }

    fn apply(&mut self, packet: ClientboundGamePacket<'_>) {
        let seen = match packet {
            ClientboundGamePacket::Base(BasePacket::ClientboundBundleDelimiter(_)) => {
                Seen::Delimiter
            }
            ClientboundGamePacket::ClientboundAddEntity(add) => Seen::Add {
                id: add.id.0,
                kind: add.kind,
                data: add.data.0,
                position: add.pos,
                velocity: add.movement.0,
            },
            ClientboundGamePacket::ClientboundSetEntityMotion(motion) => Seen::Motion {
                id: motion.entity_id.0,
                velocity: motion.velocity.0,
            },
            ClientboundGamePacket::ClientboundMoveEntityPos(step) => Seen::Move {
                id: step.entity_id.0,
                path: match step.delta {
                    VecDelta::Linear(_) => Path::Linear,
                    VecDelta::Stepped { .. } => Path::Stepped,
                },
            },
            ClientboundGamePacket::ClientboundEntityPositionSync(sync) => Seen::Move {
                id: sync.entity_id.0,
                path: match sync.position {
                    PositionPath::Linear(_) => Path::Linear,
                    PositionPath::Stepped { .. } => Path::Stepped,
                },
            },
            ClientboundGamePacket::ClientboundRemoveEntities(remove) => Seen::Remove {
                ids: remove.entity_ids.iter().map(|id| id.0).collect(),
            },
            _ => return,
        };
        self.seen.push(seen);
    }
}

fn block_at(server: &mut PlayServer, pos: BlockPos) -> Option<BlockStateId> {
    let world = server.world();
    let index = world
        .query_filtered::<&SectionIndex, With<Dimension>>()
        .single(world)
        .unwrap();
    index
        .blocks(SectionPos::from(pos))
        .map(|blocks| blocks.get(LocalPos::from(pos)))
}

fn place(server: &mut PlayServer, pos: BlockPos, state: BlockStateId) {
    let world = server.world();
    let dimension = world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .unwrap();
    world
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension,
            pos,
            new_state: state,
            flags: BlockUpdateFlags::all(),
            recursion_left: 1,
        });
}

fn falling_block_add(seen: &[Seen], start: DVec3) -> usize {
    let found = seen.iter().enumerate().find_map(|(at, seen)| match *seen {
        Seen::Add {
            kind: EntityType::FallingBlock,
            position,
            velocity,
            ..
        } => Some((at, position, velocity)),
        _ => None,
    });
    let Some((add, position, velocity)) = found else {
        panic!("a falling block is added: {seen:#?}");
    };
    assert_eq!(
        (position, velocity),
        (start, DVec3::ZERO),
        "FBK-SPN-002: the add carries the start and no velocity: {seen:#?}"
    );
    add
}

#[test]
fn fbk_net_001_requested_sand_falls_to_a_viewer_and_lands_on_the_floor() {
    let mut app = host_app::make_host_app();
    app.add_plugins(VanillaEntityPhysicsPlugin);
    app.add_plugins(VanillaFallingBlockPlugin);
    let mut server = PlayServer::start(app, &[], 0);
    load_stone_column(&mut server);
    let (viewer_anchor, _) = server.join(VIEWER, "viewer");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(viewer_anchor, 2));

    let sand = crate::support::standalone_corpus()
        .0
        .default_state("minecraft:sand")
        .unwrap();
    let start = BlockPos::new(4, 100, 4);
    place(&mut server, start, sand);
    viewer.receive(server.ticks(viewer_anchor, 2));
    assert_eq!(block_at(&mut server, start), Some(sand), "the sand stands");

    server
        .world()
        .resource_mut::<Messages<FallRequest>>()
        .write(FallRequest {
            pos: start,
            state: sand,
        });
    viewer.receive(server.ticks(viewer_anchor, 100));

    let seen = &viewer.seen;
    let add = falling_block_add(seen, start.bottom_centre());
    let Seen::Add { id, data, .. } = seen[add] else {
        unreachable!("the position finds an add");
    };
    assert_eq!(
        data,
        i32::from(sand.number()),
        "the add carries the state of sand: {seen:#?}"
    );
    let bundle_end = if add > 0 && seen[add - 1] == Seen::Delimiter {
        seen[add..]
            .iter()
            .position(|seen| *seen == Seen::Delimiter)
            .map_or(seen.len(), |end| add + end)
    } else {
        add + 1
    };
    assert!(
        !seen[add..bundle_end]
            .iter()
            .any(|seen| matches!(seen, Seen::Motion { .. })),
        "the spawn bundle has no motion: {seen:#?}"
    );
    let after = &seen[bundle_end..];
    assert!(
        after
            .iter()
            .any(|seen| matches!(seen, Seen::Motion { id: of, velocity } if *of == id && velocity.y < 0.0)),
        "a downward motion follows the add: {seen:#?}"
    );
    assert!(
        after
            .iter()
            .any(|seen| matches!(seen, Seen::Move { id: of, .. } if *of == id)),
        "the falling block moves: {seen:#?}"
    );
    assert!(
        !after
            .iter()
            .any(|seen| matches!(seen, Seen::Move { id: of, path: Path::Stepped } if *of == id)),
        "each move is linear: {seen:#?}"
    );
    let Some(removed) = after
        .iter()
        .position(|seen| matches!(seen, Seen::Remove { ids } if ids.contains(&id)))
    else {
        panic!("the landed falling block is removed: {seen:#?}");
    };
    assert!(
        !after[removed + 1..].iter().any(|seen| seen.names(id)),
        "nothing follows the removal: {seen:#?}"
    );

    assert_eq!(
        block_at(&mut server, start),
        Some(BlockStateId::AIR),
        "the start holds the leftover"
    );
    assert_eq!(
        block_at(&mut server, BlockPos::new(4, 64, 4)),
        Some(sand),
        "the sand lands on the stone"
    );
}
