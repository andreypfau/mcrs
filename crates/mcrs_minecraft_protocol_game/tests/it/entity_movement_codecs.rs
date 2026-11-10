//! The step count of a `VecDelta` travels in the enclosing packet's properties
//! field rather than in front of the steps, so these two packets carry
//! hand-written codecs that the derive cannot express.

use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    ClientboundMoveEntityRot, DeltaStep, PositionPath, PositionStep, VecDelta,
};

fn round_trip<'a, P: Encode + Decode<'a> + std::fmt::Debug>(packet: &P, buf: &'a mut Vec<u8>) -> P {
    packet.encode(&mut *buf).expect("encode");
    let mut r: &[u8] = buf;
    let decoded = P::decode(&mut r).expect("decode");
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    decoded
}

#[test]
fn movement_packets_round_trip_with_the_step_count_in_the_properties() {
    {
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
    {
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
            delta: VecDelta::Stepped {
                steps: steps.clone(),
                start_immediately: false,
            },
            y_rot: ByteAngle(64),
            x_rot: ByteAngle(200),
            on_ground: false,
        };
        let mut buf = Vec::new();
        let decoded = round_trip(&packet, &mut buf);
        assert_eq!(
            decoded.delta,
            VecDelta::Stepped {
                steps,
                start_immediately: false,
            }
        );
        assert!(!decoded.on_ground);
        assert_eq!(decoded.y_rot.0, 64);
        assert_eq!(decoded.x_rot.0, 200);
        assert_eq!(
            buf[1], 4,
            "step count must be packed above the on-ground bit"
        );
    }
    {
        for position in [
            PositionPath::Linear(DVec3::new(1.0, 2.0, 3.0)),
            PositionPath::Stepped {
                steps: vec![PositionStep {
                    position: DVec3::new(4.0, 5.0, 6.0),
                    tick_offset: VarInt(3),
                }],
                start_immediately: false,
            },
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
fn the_start_flag_comes_before_the_steps_of_a_delta_and_after_the_steps_of_a_sync() {
    let step = DeltaStep {
        ticks: VarInt(3),
        delta: [10, 0, -10],
    };
    let packet = ClientboundMoveEntityPos {
        entity_id: VarInt(7),
        delta: VecDelta::Stepped {
            steps: vec![step],
            start_immediately: true,
        },
        on_ground: false,
    };
    let mut expected = Vec::new();
    VarInt(7).encode(&mut expected).unwrap();
    VarInt(1 << 1).encode(&mut expected).unwrap();
    true.encode(&mut expected).unwrap();
    step.encode(&mut expected).unwrap();
    let mut buf = Vec::new();
    let decoded = round_trip(&packet, &mut buf);
    assert_eq!(buf, expected);
    assert_eq!(decoded.delta, packet.delta);

    let step = PositionStep {
        position: DVec3::new(4.0, 5.0, 6.0),
        tick_offset: VarInt(3),
    };
    let packet = ClientboundEntityPositionSync {
        entity_id: VarInt(7),
        position: PositionPath::Stepped {
            steps: vec![step.clone()],
            start_immediately: true,
        },
        look: Look::default(),
        on_ground: false,
    };
    let mut expected = Vec::new();
    VarInt(7).encode(&mut expected).unwrap();
    VarInt(1).encode(&mut expected).unwrap();
    VarInt(1).encode(&mut expected).unwrap();
    step.encode(&mut expected).unwrap();
    true.encode(&mut expected).unwrap();
    Look::default().encode(&mut expected).unwrap();
    false.encode(&mut expected).unwrap();
    let mut buf = Vec::new();
    let decoded = round_trip(&packet, &mut buf);
    assert_eq!(buf, expected);
    assert_eq!(decoded.position, packet.position);
}

#[test]
fn a_stepped_sync_path_with_no_step_is_rejected() {
    let mut buf = Vec::new();
    VarInt(1).encode(&mut buf).unwrap();
    VarInt(0).encode(&mut buf).unwrap();
    false.encode(&mut buf).unwrap();
    let mut r: &[u8] = &buf;
    assert!(PositionPath::decode(&mut r).is_err());
}

/// TRK-WIRE-002: TRK-E2 captured the body `01 5B 00` after the id: ground 1, yaw 91, pitch 0.
#[test]
fn trk_wire_002_a_rotation_writes_the_ground_flag_before_the_angles() {
    let packet = ClientboundMoveEntityRot {
        entity_id: VarInt(65),
        on_ground: true,
        y_rot: ByteAngle(91),
        x_rot: ByteAngle(0),
    };
    let mut buf = Vec::new();
    round_trip(&packet, &mut buf);
    assert_eq!(buf, [65, 0x01, 0x5B, 0x00]);
}

/// TRK-WIRE-004: an angle packs to the floor of its step, not to the nearest step.
#[test]
fn trk_wire_004_an_angle_packs_to_the_floor_of_its_step() {
    for (degrees, byte) in [(0.0, 0), (1.0, 0), (1.4, 0), (359.9, 255), (-1.0, 255)] {
        assert_eq!(
            ByteAngle::from_degrees(degrees),
            ByteAngle(byte),
            "{degrees}"
        );
    }
}
