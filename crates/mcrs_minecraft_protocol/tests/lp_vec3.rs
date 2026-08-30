use bevy_math::DVec3;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol::{ByteAngle, Decode, Encode, LpVec3, VarInt};
use uuid::Uuid;

fn encode(vec: DVec3) -> Vec<u8> {
    let mut buf = Vec::new();
    LpVec3(vec).encode(&mut buf).expect("encode");
    buf
}

fn round_trip(vec: DVec3) -> (DVec3, Vec<u8>) {
    let buf = encode(vec);
    let mut r: &[u8] = &buf;
    let decoded = LpVec3::decode(&mut r).expect("decode");
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    assert_eq!(encode(decoded.0), buf, "re-encode must be byte identical");
    (decoded.0, buf)
}

#[test]
fn a_vector_below_the_minimum_collapses_to_a_single_zero_byte() {
    for vec in [
        DVec3::ZERO,
        DVec3::splat(1e-6),
        DVec3::new(0.0, -LpVec3::ABS_MIN_VALUE / 2.0, 0.0),
    ] {
        let (decoded, buf) = round_trip(vec);
        assert_eq!(buf, [0]);
        assert_eq!(decoded, DVec3::ZERO);
    }
}

#[test]
fn the_smallest_representable_vector_still_encodes_in_full() {
    let buf = encode(DVec3::new(LpVec3::ABS_MIN_VALUE, 0.0, 0.0));
    assert_eq!(buf.len(), 6);
    assert_ne!(buf[0], 0);
}

#[test]
fn known_vectors_encode_to_known_bytes() {
    assert_eq!(
        encode(DVec3::new(1.0, 0.0, 0.0)),
        [0xF1, 0xFF, 0x7F, 0xFE, 0xFF, 0xFF]
    );
    assert_eq!(encode(DVec3::splat(-1.0)), [0x01, 0, 0, 0, 0, 0]);
    assert_eq!(
        encode(DVec3::new(3.0, 0.0, 0.0)),
        [0xF3, 0xFF, 0x7F, 0xFE, 0xFF, 0xFF]
    );
    assert_eq!(
        encode(DVec3::new(4.0, 0.0, 0.0)),
        [0xF4, 0xFF, 0x7F, 0xFE, 0xFF, 0xFF, 0x01]
    );
    assert_eq!(
        encode(DVec3::new(100.0, -50.0, 25.0)),
        [0xF4, 0xFF, 0x9F, 0xFE, 0x80, 0x03, 0x19]
    );
    assert_eq!(
        encode(DVec3::new(1e6, 0.0, 0.0)),
        [0xF4, 0xFF, 0x7F, 0xFE, 0xFF, 0xFF, 0x90, 0xA1, 0x0F]
    );
}

#[test]
fn the_continuation_bit_turns_on_above_a_scale_of_three() {
    for (magnitude, len, continuation) in [
        (1.0, 6, false),
        (3.0, 6, false),
        (3.5, 7, true),
        (4.0, 7, true),
    ] {
        let (_, buf) = round_trip(DVec3::new(magnitude, 0.0, 0.0));
        assert_eq!(buf.len(), len, "magnitude {magnitude}");
        assert_eq!(buf[0] & 4 == 4, continuation, "magnitude {magnitude}");
        assert_eq!(
            buf[0] & 3,
            magnitude.ceil() as u8 & 3,
            "magnitude {magnitude}"
        );
    }
}

#[test]
fn every_value_class_round_trips_within_one_quantization_step() {
    for vec in [
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(0.5, -0.5, 0.25),
        DVec3::splat(-1.0),
        DVec3::new(2.5, -3.0, 1.0),
        DVec3::new(3.0, 3.0, -3.0),
        DVec3::new(3.5, -0.125, 2.0),
        DVec3::new(4.0, 4.0, 4.0),
        DVec3::new(100.0, -50.0, 25.0),
        DVec3::new(1e6, -1.0, 0.0),
        DVec3::new(0.0, 78.4, -0.03),
        DVec3::splat(LpVec3::ABS_MAX_VALUE),
        DVec3::new(LpVec3::ABS_MAX_VALUE, -LpVec3::ABS_MAX_VALUE, 0.0),
    ] {
        let (decoded, _) = round_trip(vec);
        let scale = vec.abs().max_element().ceil();
        let step = scale * 2.0 / 32766.0;
        for axis in 0..3 {
            let error = (decoded[axis] - vec[axis]).abs();
            assert!(
                error <= step / 2.0 + f64::EPSILON * scale,
                "{vec:?} decoded as {decoded:?}: axis {axis} is off by {error}, step is {step}"
            );
        }
    }
}

#[test]
fn out_of_range_components_are_clamped_and_nan_becomes_zero() {
    let (decoded, _) = round_trip(DVec3::new(1e30, -1e30, f64::NAN));
    assert_eq!(
        decoded,
        DVec3::new(LpVec3::ABS_MAX_VALUE, -LpVec3::ABS_MAX_VALUE, 0.0)
    );

    let (decoded, buf) = round_trip(DVec3::new(f64::NAN, 1.0, f64::NAN));
    assert_eq!(decoded, DVec3::new(0.0, 1.0, 0.0));
    assert_eq!(buf.len(), 6);
}

#[test]
fn the_largest_scale_travels_as_a_five_byte_varint() {
    let (decoded, buf) = round_trip(DVec3::new(LpVec3::ABS_MAX_VALUE, 0.0, 0.0));
    assert_eq!(buf.len(), 11);
    assert_eq!(&buf[6..], [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert_eq!(decoded.x, LpVec3::ABS_MAX_VALUE);
}

#[test]
fn add_entity_carries_the_movement_vector() {
    let packet = ClientboundAddEntity {
        id: VarInt(42),
        uuid: Uuid::from_u128(7),
        kind: VarInt(3),
        pos: DVec3::new(1.5, 64.0, -2.5),
        movement: LpVec3(DVec3::new(0.25, -0.5, 4.0)),
        yaw: ByteAngle(12),
        pitch: ByteAngle(200),
        head_yaw: ByteAngle(30),
        data: VarInt(0),
    };

    let mut buf = Vec::new();
    packet.encode(&mut buf).expect("encode");
    let mut r: &[u8] = &buf;
    let decoded = ClientboundAddEntity::decode(&mut r).expect("decode");
    assert!(r.is_empty(), "{} trailing bytes", r.len());

    let mut again = Vec::new();
    decoded.encode(&mut again).expect("re-encode");
    assert_eq!(again, buf);
    assert!((decoded.movement.0.z - 4.0).abs() < 1e-9);
}
