use mcrs_minecraft_bedrock_wire::{Error, VarI32, VarI64, VarU32, VarU64, from_slice, to_bytes};

#[test]
fn var_u32_round_trip() {
    let cases = [0, 1, 127, 128, 255, 256, 16383, 16384, 2097151, u32::MAX];
    for val in cases {
        let bytes = to_bytes(&VarU32(val)).unwrap();
        assert_eq!(bytes.len(), VarU32(val).encoded_len());
        let decoded: VarU32 = from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, val);
    }
}

#[test]
fn var_i32_zigzag_encoding_values() {
    assert_eq!(VarI32::zigzag_encode(0), 0);
    assert_eq!(VarI32::zigzag_encode(-1), 1);
    assert_eq!(VarI32::zigzag_encode(1), 2);
    assert_eq!(VarI32::zigzag_encode(-2), 3);
    assert_eq!(VarI32::zigzag_encode(2), 4);
    assert_eq!(VarI32::zigzag_encode(i32::MAX), 0xFFFFFFFE);
    assert_eq!(VarI32::zigzag_encode(i32::MIN), 0xFFFFFFFF);
}

#[test]
fn var_i32_zigzag_round_trip() {
    let cases = [
        0,
        -1,
        1,
        -2,
        2,
        -100,
        100,
        -32768,
        32767,
        i32::MIN,
        i32::MAX,
    ];
    for val in cases {
        let bytes = to_bytes(&VarI32(val)).unwrap();
        assert_eq!(bytes.len(), VarI32(val).encoded_len());
        let decoded: VarI32 = from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, val);
    }
}

#[test]
fn var_u64_round_trip() {
    let cases = [0, 1, 127, 128, u64::MAX];
    for val in cases {
        let bytes = to_bytes(&VarU64(val)).unwrap();
        assert_eq!(bytes.len(), VarU64(val).encoded_len());
        let decoded: VarU64 = from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, val);
    }
}

#[test]
fn var_i64_zigzag_round_trip() {
    let cases = [0, -1, 1, -2, 2, -1000000, 1000000, i64::MIN, i64::MAX];
    for val in cases {
        let bytes = to_bytes(&VarI64(val)).unwrap();
        assert_eq!(bytes.len(), VarI64(val).encoded_len());
        let decoded: VarI64 = from_slice(&bytes).unwrap();
        assert_eq!(decoded.0, val);
    }
}

#[test]
fn var_u32_rejects_overflow_bits() {
    // 5th byte with high bits set (0xF0)
    let bad = [0x80, 0x80, 0x80, 0x80, 0xF0];
    let err = from_slice::<VarU32>(&bad).unwrap_err();
    assert_eq!(err, Error::VarIntOverflow);
}

#[test]
fn little_endian_primitives_round_trip() {
    assert_eq!(to_bytes(&0x1234u16).unwrap(), [0x34, 0x12]);
    assert_eq!(to_bytes(&0x12345678u32).unwrap(), [0x78, 0x56, 0x34, 0x12]);
    assert_eq!(
        to_bytes(&0x123456789ABCDEF0u64).unwrap(),
        [0xF0, 0xDE, 0xBC, 0x9A, 0x78, 0x56, 0x34, 0x12]
    );

    let val_f32 = core::f32::consts::PI;
    let bytes_f32 = to_bytes(&val_f32).unwrap();
    let decoded_f32: f32 = from_slice(&bytes_f32).unwrap();
    assert_eq!(decoded_f32, val_f32);

    assert_eq!(to_bytes(&true).unwrap(), [1]);
    assert_eq!(to_bytes(&false).unwrap(), [0]);
    assert!(from_slice::<bool>(&[1]).unwrap());
    assert!(!from_slice::<bool>(&[0]).unwrap());
    assert_eq!(from_slice::<bool>(&[2]).unwrap_err(), Error::InvalidBool);
}

#[test]
fn bedrock_string_round_trip() {
    let original = "Hello Bedrock 🌍!";
    let bytes = to_bytes(&original.to_string()).unwrap();
    let decoded: String = from_slice(&bytes).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn vec_round_trip() {
    let items = vec![10u32, 20u32, 30u32];
    let bytes = to_bytes(&items).unwrap();
    let decoded: Vec<u32> = from_slice(&bytes).unwrap();
    assert_eq!(decoded, items);
}

#[test]
fn trailing_bytes_rejected() {
    let bytes = [0x01, 0x02];
    let err = from_slice::<u8>(&bytes).unwrap_err();
    assert_eq!(err, Error::TrailingBytes);
}
