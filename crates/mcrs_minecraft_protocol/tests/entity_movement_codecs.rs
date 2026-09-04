//! The step count of a `VecDelta` travels in the enclosing packet's properties
//! field rather than in front of the steps, so these two packets carry
//! hand-written codecs that the derive cannot express.

use bevy_math::DVec3;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    DeltaStep, PositionPath, PositionStep, VecDelta,
};
use mcrs_minecraft_protocol::{ByteAngle, Decode, Encode, Look, VarInt};

fn round_trip<'a, P: Encode + Decode<'a> + std::fmt::Debug>(packet: &P, buf: &'a mut Vec<u8>) -> P {
    packet.encode(&mut *buf).expect("encode");
    let mut r: &[u8] = buf;
    let decoded = P::decode(&mut r).expect("decode");
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    decoded
}

#[test]
fn linear_move_entity_pos_round_trips() {
    let packet = ClientboundMoveEntityPos {
        entity_id: VarInt(300),
        delta: VecDelta::Linear([1, -2, 3]),
        on_ground: true,
    };
    let mut buf = Vec::new();
    let decoded = round_trip(&packet, &mut buf);
    assert_eq!(decoded.delta, VecDelta::Linear([1, -2, 3]));
    assert!(decoded.on_ground);
    // entity id, properties(on_ground | 0 << 1), then three big-endian shorts.
    assert_eq!(buf, [0xAC, 0x02, 0x01, 0, 1, 0xFF, 0xFE, 0, 3]);
}

#[test]
fn stepped_move_entity_pos_rot_round_trips() {
    let steps = vec![
        DeltaStep {
            ticks: VarInt(1),
            delta: [10, 0, -10],
        },
        DeltaStep {
            ticks: VarInt(2),
            delta: [0, 5, 0],
        },
    ];
    let packet = ClientboundMoveEntityPosRot {
        entity_id: VarInt(7),
        delta: VecDelta::Stepped(steps.clone()),
        y_rot: ByteAngle(64),
        x_rot: ByteAngle(200),
        on_ground: false,
    };
    let mut buf = Vec::new();
    let decoded = round_trip(&packet, &mut buf);
    assert_eq!(decoded.delta, VecDelta::Stepped(steps));
    assert!(!decoded.on_ground);
    assert_eq!(decoded.y_rot.0, 64);
    assert_eq!(decoded.x_rot.0, 200);
    assert_eq!(
        buf[1], 4,
        "step count must be packed above the on-ground bit"
    );
}

#[test]
fn a_step_count_larger_than_the_input_is_rejected() {
    let mut buf = Vec::new();
    VarInt(7).encode(&mut buf).unwrap();
    VarInt(1 | 4096 << 1).encode(&mut buf).unwrap();
    let mut r: &[u8] = &buf;
    assert!(ClientboundMoveEntityPos::decode(&mut r).is_err());
}

#[test]
fn entity_position_sync_carries_a_position_path() {
    for position in [
        PositionPath::Linear(DVec3::new(1.0, 2.0, 3.0)),
        PositionPath::Stepped(vec![PositionStep {
            position: DVec3::new(4.0, 5.0, 6.0),
            tick_offset: VarInt(3),
        }]),
    ] {
        let packet = ClientboundEntityPositionSync {
            entity_id: VarInt(1),
            position: position.clone(),
            look: Look {
                yaw: 90.0,
                pitch: -45.0,
            },
            on_ground: true,
        };
        let mut buf = Vec::new();
        let decoded = round_trip(&packet, &mut buf);
        assert_eq!(decoded.position, position);
    }
}
