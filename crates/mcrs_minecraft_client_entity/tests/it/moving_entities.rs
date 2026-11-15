use glam::DVec3;
use mcrs_minecraft_client_entity::Velocity;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundSetEntityMotion,
    PositionPath, VecDelta,
};

use crate::{Client, add_entity};

const ID: i32 = 7;

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
        let label = format!(
            "{:?} {}",
            case.kind,
            match case.update {
                Update::Move { blocks } => format!("move {blocks}"),
                Update::Sync { z } => format!("sync {z}"),
            }
        );
        match case.update {
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

/// The velocity that the client decodes from `velocity` on the wire.
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
        .get::<Velocity>(entity)
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
