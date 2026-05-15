use mcrs_protocol::{ColumnPos, Decode, Encode};

#[test]
fn column_pos_encode_byte_layout() {
    let pos = ColumnPos { x: 3, z: -7 };
    let mut buf = Vec::new();
    pos.encode(&mut buf).expect("encode");
    assert_eq!(
        buf,
        vec![0x00, 0x00, 0x00, 0x03, 0xFF, 0xFF, 0xFF, 0xF9],
        "wire layout must be big-endian i32 x then big-endian i32 z"
    );
}

#[test]
fn column_pos_decode_round_trip() {
    let cases = [
        ColumnPos { x: 0, z: 0 },
        ColumnPos { x: 1, z: -1 },
        ColumnPos { x: 3, z: -7 },
        ColumnPos { x: -42, z: 42 },
        ColumnPos {
            x: i32::MAX,
            z: i32::MIN,
        },
        ColumnPos {
            x: i32::MIN,
            z: i32::MAX,
        },
    ];
    for case in cases {
        let mut buf = Vec::new();
        case.encode(&mut buf).expect("encode");
        let mut slice: &[u8] = &buf;
        let decoded = ColumnPos::decode(&mut slice).expect("decode");
        assert_eq!(decoded, case, "round-trip mismatch for {case:?}");
        assert!(slice.is_empty(), "decode must consume every byte for {case:?}");
    }
}

#[test]
fn column_pos_wire_size_is_eight_bytes() {
    for pos in [
        ColumnPos::default(),
        ColumnPos::new(i32::MAX, i32::MIN),
    ] {
        let mut buf = Vec::new();
        pos.encode(&mut buf).expect("encode");
        assert_eq!(buf.len(), 8, "ColumnPos must encode to exactly 8 bytes");
    }
}
