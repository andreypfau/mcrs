use glam::DVec3;
use mcrs_minecraft_client_entity::TrackedVelocity;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityRot,
    ClientboundSetEntityMotion, ClientboundTeleportEntity, PositionPath, VecDelta,
};
use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3, PositionFlag};

use crate::{Client, add_entity};

const ID: i32 = 7;

#[derive(Clone, Copy, Debug)]
enum Update {
    Move { blocks: f64 },
    Sync { z: f64 },
}

struct Case {
    kind: EntityType,
    update: Update,
    previous_after_packet: f64,
    after_ticks: [f64; 4],
}

fn receive_update(client: &mut Client, update: Update) {
    match update {
        Update::Move { blocks } => client.receive(ClientboundMoveEntityPos {
            entity_id: VarInt(ID),
            delta: VecDelta::Linear([0, 0, (blocks * 4096.0) as i16]),
            on_ground: false,
        }),
        Update::Sync { z } => client.receive(ClientboundEntityPositionSync {
            entity_id: VarInt(ID),
            position: PositionPath::Linear(DVec3::new(0.5, 64.0, z)),
            look: Look::default(),
            on_ground: false,
        }),
    }
}

fn z_and_previous(client: &mut Client) -> (f64, f64) {
    client.inputs(ID, |inputs| {
        (
            inputs.motion.current.position.z,
            inputs.motion.previous.position.z,
        )
    })
}

#[test]
fn a_position_update_moves_an_item_at_once_and_an_orb_in_three_steps() {
    use EntityType::{ExperienceOrb, Item};
    let one_third = 64.0 / 3.0;
    let cases = [
        Case {
            kind: Item,
            update: Update::Move { blocks: 1.5 },
            previous_after_packet: 0.0,
            after_ticks: [1.5, 1.5, 1.5, 1.5],
        },
        Case {
            kind: ExperienceOrb,
            update: Update::Move { blocks: 1.5 },
            previous_after_packet: 0.0,
            after_ticks: [0.0, 0.5, 1.0, 1.5],
        },
        Case {
            kind: Item,
            update: Update::Sync { z: 64.0 },
            previous_after_packet: 0.0,
            after_ticks: [64.0, 64.0, 64.0, 64.0],
        },
        Case {
            kind: ExperienceOrb,
            update: Update::Sync { z: 64.0 },
            previous_after_packet: 0.0,
            after_ticks: [0.0, one_third, 2.0 * one_third, 64.0],
        },
        Case {
            kind: Item,
            update: Update::Sync { z: 64.5 },
            previous_after_packet: 64.5,
            after_ticks: [64.5, 64.5, 64.5, 64.5],
        },
        Case {
            kind: ExperienceOrb,
            update: Update::Sync { z: 64.5 },
            previous_after_packet: 64.5,
            after_ticks: [64.5, 64.5, 64.5, 64.5],
        },
    ];
    for case in cases {
        let mut client = Client::new();
        client.receive(add_entity(
            ID,
            case.kind,
            DVec3::new(0.5, 64.0, 0.0),
            0.0,
            0.0,
        ));
        client.frame();
        client.tick();
        let label = format!("{:?} {:?}", case.kind, case.update);
        receive_update(&mut client, case.update);
        client.frame();
        let (z, previous) = z_and_previous(&mut client);
        assert_eq!(previous, case.previous_after_packet, "{label}: previous");
        let mut seen = vec![z];
        for _ in 0..3 {
            client.tick();
            seen.push(z_and_previous(&mut client).0);
        }
        for (seen, expected) in seen.iter().zip(case.after_ticks) {
            assert!(
                (seen - expected).abs() < 1e-9,
                "{label}: {seen:?} != {:?}",
                case.after_ticks
            );
        }
    }
}

#[test]
fn tick_after_an_instant_item_move_starts_from_the_new_position() {
    for (update, z) in [
        (Update::Move { blocks: 1.5 }, 1.5),
        (Update::Sync { z: 64.0 }, 64.0),
    ] {
        let mut client = Client::new();
        client.receive(add_entity(
            ID,
            EntityType::Item,
            DVec3::new(0.5, 64.0, 0.0),
            0.0,
            0.0,
        ));
        client.frame();
        client.tick();
        receive_update(&mut client, update);
        client.tick();
        assert_eq!(z_and_previous(&mut client), (z, z), "{update:?}");
    }
}

fn wire(velocity: DVec3) -> DVec3 {
    let mut data = Vec::new();
    LpVec3(velocity)
        .encode(&mut data)
        .expect("the velocity encodes");
    LpVec3::decode(&mut data.as_slice())
        .expect("the velocity decodes")
        .0
}

fn set_motion(client: &mut Client, velocity: DVec3) {
    client.receive(ClientboundSetEntityMotion {
        entity_id: VarInt(ID),
        velocity: LpVec3(velocity),
    });
}

fn living_velocity(client: &Client) -> DVec3 {
    let entity = client.entity(ID).expect("the entity was added");
    client
        .app
        .world()
        .get::<TrackedVelocity>(entity)
        .expect("a living entity has a velocity")
        .value
}

#[test]
fn the_velocity_follows_the_packet_rule_of_each_entity_class() {
    let start = DVec3::new(0.5, 64.0, 0.0);

    let mut item = Client::new();
    item.receive(add_entity(ID, EntityType::Item, start, 0.0, 0.0));
    set_motion(&mut item, DVec3::new(0.0, 0.5, 0.0));
    let entity = item.entity(ID).expect("the item was added");
    let state = item
        .app
        .world()
        .get::<MotionState>(entity)
        .expect("an item is simulated");
    assert_eq!(state.0.body.velocity, wire(DVec3::new(0.0, 0.5, 0.0)));

    let mut pig = Client::new();
    pig.receive(add_entity(ID, EntityType::Pig, start, 0.0, 0.0));
    set_motion(&mut pig, DVec3::X);
    for _ in 0..287 {
        pig.tick();
    }
    assert!(
        living_velocity(&pig).x > 0.0,
        "the decay keeps 0.98^287 above the cutoff"
    );
    pig.tick();
    assert_eq!(living_velocity(&pig), DVec3::ZERO);
    set_motion(&mut pig, DVec3::X);
    pig.receive(ClientboundMoveEntityPos {
        entity_id: VarInt(ID),
        delta: VecDelta::Linear([0, 0, 4096]),
        on_ground: false,
    });
    pig.tick();
    pig.tick();
    assert_eq!(
        living_velocity(&pig),
        wire(DVec3::X),
        "an active interpolation stops the decay"
    );
    pig.tick();
    assert_eq!(living_velocity(&pig), wire(DVec3::X) * 0.98);

    let mut player = Client::new();
    player.receive(add_entity(ID, EntityType::Player, start, 0.0, 0.0));
    set_motion(&mut player, DVec3::new(0.0, 0.0, 0.75));
    assert_eq!(living_velocity(&player), DVec3::ZERO);
    let mut seen = Vec::new();
    for _ in 0..5 {
        player.tick();
        seen.push(living_velocity(&player).z);
    }
    let target = wire(DVec3::new(0.0, 0.0, 0.75)).z;
    let thirds = [1.0, 2.0, 3.0, 3.0, 3.0].map(|thirds| target * thirds / 3.0);
    for (seen, expected) in seen.iter().zip(thirds) {
        assert!((seen - expected).abs() < 1e-12, "{seen} != {expected}");
    }
}

#[test]
fn a_teleport_that_turns_the_entity_rotates_its_velocity() {
    let start = DVec3::new(0.5, 64.0, 0.0);
    let mut client = Client::new();
    client.receive(add_entity(ID, EntityType::Pig, start, 0.0, 0.0));
    set_motion(&mut client, DVec3::X);
    client.receive(ClientboundTeleportEntity {
        entity_id: VarInt(ID),
        position: start,
        velocity: DVec3::new(0.25, 0.0, 0.5),
        look: Look {
            yaw: 90.0,
            pitch: 0.0,
        },
        relatives: [
            PositionFlag::YRot,
            PositionFlag::DeltaZ,
            PositionFlag::RotateDelta,
        ]
        .into_iter()
        .collect(),
        on_ground: false,
    });
    let expected = DVec3::new(0.25, 0.0, wire(DVec3::X).x + 0.5);
    assert!(
        living_velocity(&client).distance(expected) < 1e-6,
        "a turn by 90 degrees rotates +x to +z before the relative z adds: {} != {expected}",
        living_velocity(&client)
    );
}

enum TeleportResult {
    Reaches { z: f64 },
    Snaps { z: f64 },
}

#[test]
fn a_relative_teleport_during_an_interpolation_starts_from_its_target() {
    let cases = [
        (0.5, TeleportResult::Reaches { z: 1.5 }),
        (100.0, TeleportResult::Snaps { z: 101.0 }),
    ];
    for (dz, result) in cases {
        let mut client = Client::new();
        client.receive(add_entity(
            ID,
            EntityType::Pig,
            DVec3::new(0.5, 64.0, 0.0),
            0.0,
            0.0,
        ));
        client.tick();
        client.receive(ClientboundMoveEntityPos {
            entity_id: VarInt(ID),
            delta: VecDelta::Linear([0, 0, 4096]),
            on_ground: false,
        });
        client.tick();
        let (z_before, previous_before) = z_and_previous(&mut client);
        assert!(
            z_before > 0.0 && z_before < 1.0,
            "{dz}: the interpolation to z 1 is active: {z_before}"
        );
        client.receive(ClientboundTeleportEntity {
            entity_id: VarInt(ID),
            position: DVec3::new(0.0, 0.0, dz),
            velocity: DVec3::ZERO,
            look: Look::default(),
            relatives: [PositionFlag::X, PositionFlag::Y, PositionFlag::Z]
                .into_iter()
                .collect(),
            on_ground: false,
        });
        match result {
            TeleportResult::Reaches { z } => {
                for _ in 0..10 {
                    client.tick();
                }
                assert_eq!(z_and_previous(&mut client).0, z, "{dz}");
            }
            TeleportResult::Snaps { z } => {
                let (z_after, previous_after) = z_and_previous(&mut client);
                assert_eq!(z_after, z, "{dz}");
                assert!(
                    ((previous_after - z_after) - (previous_before - z_before)).abs() < 1e-9,
                    "{dz}: the previous z keeps its offset: {previous_after} - {z_after}"
                );
            }
        }
    }
}

struct OrbTurn {
    start_yaw: f32,
    yaw: f32,
    pitch: f32,
    yaws: [f32; 3],
    pitches: [f32; 3],
}

#[test]
fn the_orb_rotation_wraps_the_yaw_and_clamps_the_pitch() {
    let cases = [
        OrbTurn {
            start_yaw: 168.75,
            yaw: -168.75,
            pitch: 99.84375,
            yaws: [176.25, 183.75, 191.25],
            pitches: [33.28125, 66.5625, 90.0],
        },
        OrbTurn {
            start_yaw: -168.75,
            yaw: 168.75,
            pitch: -99.84375,
            yaws: [-176.25, -183.75, -191.25],
            pitches: [-33.28125, -66.5625, -90.0],
        },
    ];
    for case in cases {
        let mut client = Client::new();
        client.receive(add_entity(
            ID,
            EntityType::ExperienceOrb,
            DVec3::new(0.5, 64.0, 0.0),
            case.start_yaw,
            0.0,
        ));
        client.tick();
        client.receive(ClientboundMoveEntityRot {
            entity_id: VarInt(ID),
            on_ground: false,
            y_rot: ByteAngle::from_degrees(case.yaw),
            x_rot: ByteAngle::from_degrees(case.pitch),
        });
        for (yaw, pitch) in case.yaws.into_iter().zip(case.pitches) {
            client.tick();
            let seen = client.inputs(ID, |inputs| {
                (inputs.motion.current.yaw, inputs.motion.current.pitch)
            });
            assert_eq!(
                seen,
                (yaw, pitch),
                "from {} to {}",
                case.start_yaw,
                case.yaw
            );
        }
    }
}
