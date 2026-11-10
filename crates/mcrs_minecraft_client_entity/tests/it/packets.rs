use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundDamageEvent, ClientboundEntityEvent, ClientboundEntityPositionSync,
    ClientboundMoveEntityPos, ClientboundRemoveEntities, ClientboundRotateHead,
    ClientboundTeleportEntity, ClientboundTickingState, ClientboundTickingStep, PositionPath,
    VecDelta,
};
use mcrs_minecraft_protocol_game::{ByteAngle, PositionFlags};

use crate::{Client, add_entity};

const PIG: i32 = 5;

fn pig_at(z: f64) -> Client {
    let mut client = Client::new();
    client.receive(add_entity(
        PIG,
        EntityType::Pig,
        DVec3::new(0.5, 64.0, z),
        0.0,
        0.0,
    ));
    client
}

fn z(client: &mut Client) -> f64 {
    client.inputs(PIG, |inputs| inputs.motion.current.position.z)
}

#[test]
fn a_relative_move_reaches_its_target_over_the_update_interval() {
    let mut client = pig_at(0.0);
    client.receive(ClientboundMoveEntityPos {
        entity_id: VarInt(PIG),
        delta: VecDelta::Linear([0, 0, 3 * 4096 / 4]),
        on_ground: true,
    });
    let mut seen = Vec::new();
    for _ in 0..4 {
        client.tick();
        seen.push(z(&mut client));
    }
    for (seen, expected) in seen.iter().zip([0.25, 0.5, 0.75, 0.75]) {
        assert!((seen - expected).abs() < 1e-6, "{seen} != {expected}");
    }
    let (base, on_ground) = client.inputs(PIG, |inputs| {
        (inputs.motion.delta_base.z, inputs.motion.on_ground)
    });
    assert_eq!(
        base, 0.75,
        "the delta base moves to the end of the path at once"
    );
    assert!(on_ground);
}

#[test]
fn a_far_position_sync_snaps_and_a_remove_despawns() {
    let mut client = pig_at(0.0);
    client.receive(ClientboundEntityPositionSync {
        entity_id: VarInt(PIG),
        position: PositionPath::Linear(DVec3::new(0.5, 64.0, 100.0)),
        look: Look::default(),
        on_ground: false,
    });
    assert_eq!(z(&mut client), 100.0);

    client.receive(ClientboundRemoveEntities {
        entity_ids: vec![VarInt(PIG)],
    });
    client.app.world_mut().flush();
    assert_eq!(client.entity(PIG), None);
}

#[test]
fn a_snap_during_a_move_carries_the_rest_of_the_path() {
    let mut client = pig_at(0.0);
    client.receive(ClientboundMoveEntityPos {
        entity_id: VarInt(PIG),
        delta: VecDelta::Linear([0, 0, 3 * 4096]),
        on_ground: true,
    });
    client.tick();
    client.receive(ClientboundTeleportEntity {
        entity_id: VarInt(PIG),
        position: DVec3::new(0.5, 64.0, 101.0),
        velocity: DVec3::ZERO,
        look: Look::default(),
        relatives: PositionFlags::default(),
        on_ground: true,
    });
    assert_eq!(z(&mut client), 101.0);
    client.tick();
    client.tick();
    let end = z(&mut client);
    assert!(
        (end - 103.0).abs() < 1e-6,
        "the end of the path moved by 100 blocks: {end}"
    );
}

#[test]
fn a_frozen_world_ticks_only_its_steps() {
    let mut client = pig_at(0.0);
    client.receive(ClientboundTickingState {
        tick_rate: 20.0,
        frozen: true,
    });
    client.tick();
    assert_eq!(client.inputs(PIG, |inputs| inputs.age.0), 0);
    assert_eq!(client.ticking().partial_tick(0.3), 1.0);
    client.receive(ClientboundTickingStep { steps: VarInt(2) });
    client.tick();
    client.tick();
    client.tick();
    assert_eq!(client.inputs(PIG, |inputs| inputs.age.0), 2);
}

#[test]
fn a_hurt_and_a_death_drive_the_timers_and_the_walk() {
    let mut client = pig_at(0.0);
    client.receive(ClientboundDamageEvent {
        entity_id: VarInt(PIG),
        damage_type: VarInt(0),
        cause_entity_id: VarInt(0),
        direct_entity_id: VarInt(0),
        source_position: None,
    });
    let frame = client.inputs(PIG, |inputs| inputs.frame(0.0));
    assert_eq!(frame.hurt_time, 10);
    assert_eq!(
        frame.walk.speed, 0.0,
        "the old speed of the frame is not changed"
    );
    client.tick();
    let frame = client.inputs(PIG, |inputs| inputs.frame(1.0));
    assert_eq!(frame.hurt_time, 9);
    assert!(
        (frame.walk.speed - 0.9).abs() < 1e-6,
        "1.5 closes 0.4 of its gap to 0: {}",
        frame.walk.speed
    );

    client.receive(ClientboundEntityEvent {
        entity_id: PIG,
        entity_status: 3,
    });
    client.tick();
    client.tick();
    let frame = client.inputs(PIG, |inputs| inputs.frame(0.5));
    assert!(!frame.alive);
    assert_eq!(frame.death_ticks, 2.5);
    assert_eq!(frame.walk.speed, 0.0);
}

#[test]
fn a_head_packet_turns_the_head_and_the_body_stays_within_the_limit() {
    let mut client = pig_at(0.0);
    client.receive(ClientboundRotateHead {
        entity_id: VarInt(PIG),
        y_head_rot: ByteAngle::from_degrees(-135.0),
    });
    for _ in 0..3 {
        client.tick();
    }
    let frame = client.inputs(PIG, |inputs| inputs.frame(1.0));
    assert!((frame.head_yaw + 135.0).abs() < 1e-4, "{}", frame.head_yaw);
    assert!((frame.body_yaw + 60.0).abs() < 1e-4, "{}", frame.body_yaw);
    assert!((frame.relative_head_yaw + 75.0).abs() < 1e-4);
}

#[test]
fn each_mob_type_of_the_server_turns_its_body_after_its_head() {
    for kind in [
        EntityType::Cow,
        EntityType::Sheep,
        EntityType::Chicken,
        EntityType::ZombifiedPiglin,
    ] {
        let mut client = Client::new();
        client.receive(add_entity(PIG, kind, DVec3::new(0.5, 64.0, 0.0), 0.0, 0.0));
        client.receive(ClientboundRotateHead {
            entity_id: VarInt(PIG),
            y_head_rot: ByteAngle::from_degrees(-135.0),
        });
        for _ in 0..3 {
            client.tick();
        }
        let frame = client.inputs(PIG, |inputs| inputs.frame(1.0));
        assert!(
            (frame.body_yaw + 60.0).abs() < 1e-4,
            "{kind:?} body yaw {}",
            frame.body_yaw
        );
        assert!(frame.eye_position.y > 64.0, "{kind:?} has an eye height");
    }
}
