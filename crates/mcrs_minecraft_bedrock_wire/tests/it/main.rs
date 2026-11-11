use core::fmt::Debug;

use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, NetworkBlockPos, NetworkChunkPos, Packet, PacketHeader,
    VarI32, VarI64, VarU32, VarU64, decode_packet, encode_packet, from_slice, to_bytes,
};

mod counting_allocator {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ALLOCATED: Cell<usize> = const { Cell::new(0) };
    }

    struct CountingAllocator;

    // SAFETY: every call forwards the caller's layout contract unchanged to the system allocator.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let _ = ALLOCATED.try_with(|bytes| bytes.set(bytes.get() + layout.size()));
            // SAFETY: the caller upholds the `GlobalAlloc::alloc` contract.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: the caller upholds the `GlobalAlloc::dealloc` contract.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAllocator = CountingAllocator;

    /// Returns the bytes that `run` allocates on this thread, with its result.
    pub fn allocated_by<T>(run: impl FnOnce() -> T) -> (usize, T) {
        let before = ALLOCATED.with(Cell::get);
        let result = run();
        (ALLOCATED.with(Cell::get) - before, result)
    }
}

fn assert_vector<T>(value: T, bytes: &[u8])
where
    T: BedrockEncode + for<'a> BedrockDecode<'a> + PartialEq + Debug,
{
    assert_eq!(to_bytes(&value), Ok(bytes.to_vec()), "encode {value:?}");
    assert_eq!(from_slice::<T>(bytes), Ok(value), "decode {bytes:02X?}");
}

#[test]
fn var_int_vectors() {
    assert_vector(VarU32(0), &[0x00]);
    assert_vector(VarU32(127), &[0x7F]);
    assert_vector(VarU32(128), &[0x80, 0x01]);
    assert_vector(VarU32(300), &[0xAC, 0x02]);
    assert_vector(VarU32(16_384), &[0x80, 0x80, 0x01]);
    assert_vector(VarU32(u32::MAX), &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert_vector(
        VarU64(1 << 63),
        &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
    );
    assert_vector(
        VarU64(u64::MAX),
        &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
    );
    assert_vector(VarI32(0), &[0x00]);
    assert_vector(VarI32(-1), &[0x01]);
    assert_vector(VarI32(1), &[0x02]);
    assert_vector(VarI32(-64), &[0x7F]);
    assert_vector(VarI32(64), &[0x80, 0x01]);
    assert_vector(VarI32(i32::MAX), &[0xFE, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert_vector(VarI32(i32::MIN), &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert_vector(
        VarI64(i64::MAX),
        &[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
    );
    assert_vector(
        VarI64(i64::MIN),
        &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
    );

    let u32_errors: [(&[u8], Error); 3] = [
        (&[0xFF, 0xFF, 0xFF, 0xFF, 0x10], Error::VarIntOverflow),
        (&[0xFF, 0xFF, 0xFF, 0xFF, 0x80], Error::VarIntOverflow),
        (&[0x80], Error::UnexpectedEof),
    ];
    for (bytes, error) in u32_errors {
        assert_eq!(
            from_slice::<VarU32>(bytes).unwrap_err(),
            error,
            "{bytes:02X?}"
        );
    }
    let u64_errors: [(&[u8], Error); 2] = [
        (
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x02],
            Error::VarIntOverflow,
        ),
        (
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x80],
            Error::VarIntOverflow,
        ),
    ];
    for (bytes, error) in u64_errors {
        assert_eq!(
            from_slice::<VarU64>(bytes).unwrap_err(),
            error,
            "{bytes:02X?}"
        );
    }
}

#[derive(Debug, PartialEq)]
struct Ping(u8);

impl Packet for Ping {
    const ID: u32 = 42;
}

impl BedrockEncode for Ping {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.0.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for Ping {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(Self(u8::decode(r)?))
    }
}

#[test]
fn position_vectors() {
    let block = |x, y, z| NetworkBlockPos { x, y, z };
    assert_vector(block(1, 64, -1), &[0x02, 0x40, 0x01]);
    assert_vector(
        block(0, -64, 0),
        &[0x00, 0xC0, 0xFF, 0xFF, 0xFF, 0x0F, 0x00],
    );
    assert_vector(NetworkChunkPos { x: 1, z: -1 }, &[0x02, 0x01]);
}

#[test]
fn packet_header_bit_layout() {
    let header = |id, sender_sub_client, target_sub_client| PacketHeader {
        id,
        sender_sub_client,
        target_sub_client,
    };
    // 42 | 1 << 10 | 2 << 12 = 9258; 1023 | 3 << 10 | 3 << 12 = 16383.
    assert_vector(header(42, 1, 2), &[0xAA, 0x48]);
    assert_vector(header(1023, 3, 3), &[0xFF, 0x7F]);
    assert_vector(header(1, 0, 0), &[0x01]);
    for overflowing in [header(1024, 0, 0), header(0, 4, 0), header(0, 0, 4)] {
        assert_eq!(
            to_bytes(&overflowing).unwrap_err(),
            Error::PacketHeaderOverflow,
            "{overflowing:?}"
        );
    }

    let mut bytes = Vec::new();
    encode_packet(&Ping(7), &mut bytes).unwrap();
    assert_eq!(bytes, [42, 7]);
    assert_eq!(
        decode_packet::<Ping>(&mut &[0xAA, 0x48, 7][..]).unwrap(),
        Ping(7)
    );
    assert_eq!(
        decode_packet::<Ping>(&mut &[43, 7][..]).unwrap_err(),
        Error::UnexpectedPacketId {
            expected: 42,
            actual: 43
        }
    );
}

#[test]
fn declared_count_does_not_preallocate_past_the_input() {
    let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x01, 0x02];
    let (allocated, result) = counting_allocator::allocated_by(|| from_slice::<Vec<u64>>(&bytes));
    assert_eq!(result.unwrap_err(), Error::UnexpectedEof);
    assert!(
        allocated <= 2 * size_of::<u64>(),
        "allocated {allocated} bytes for 2 bytes of input"
    );
}

#[test]
fn byte_array_and_str_vectors() {
    assert_eq!(to_bytes("h\u{e9}").unwrap(), [0x03, b'h', 0xC3, 0xA9]);
    assert_eq!(
        from_slice::<&str>(&[0x03, b'h', 0xC3, 0xA9]).unwrap(),
        "h\u{e9}"
    );
    assert_eq!(
        from_slice::<&str>(&[0x01, 0xFF]).unwrap_err(),
        Error::InvalidUtf8
    );
    assert_eq!(
        from_slice::<&[u8]>(&[0x03, 0x01]).unwrap_err(),
        Error::UnexpectedEof
    );
}

#[test]
fn trailing_bytes_rejected() {
    assert_eq!(
        from_slice::<u8>(&[0x01, 0x02]).unwrap_err(),
        Error::TrailingBytes
    );
    assert_eq!(
        decode_packet::<Ping>(&mut &[42, 7, 0][..]).unwrap_err(),
        Error::TrailingBytes
    );
}

#[derive(BedrockEncode, BedrockDecode, Debug, PartialEq)]
struct NamedLists<'a> {
    name: &'a str,
    #[bedrock(count = u16)]
    fixed: Vec<u8>,
    varied: Vec<u8>,
    #[bedrock(count = u16)]
    names: Vec<&'a str>,
}

#[test]
fn derived_struct_borrows_and_counts_fields() {
    let value = NamedLists {
        name: "ab",
        fixed: vec![7, 9],
        varied: vec![1],
        names: vec!["c"],
    };
    let bytes = [
        0x02, b'a', b'b', 0x02, 0x00, 7, 9, 0x01, 0x01, 0x01, 0x00, 0x01, b'c',
    ];
    assert_eq!(to_bytes(&value).unwrap(), bytes);
    assert_eq!(from_slice::<NamedLists>(&bytes).unwrap(), value);
}

#[derive(BedrockEncode, BedrockDecode, Debug, PartialEq)]
#[bedrock(id = 0x45)]
struct Move {
    x: i32,
    on_ground: bool,
}

#[test]
fn derived_packet_carries_its_id_in_the_header() {
    let packet = Move {
        x: -2,
        on_ground: true,
    };
    let bytes = [0x45, 0xFE, 0xFF, 0xFF, 0xFF, 0x01];
    let mut w = Vec::new();
    encode_packet(&packet, &mut w).unwrap();
    assert_eq!(w, bytes);
    assert_eq!(decode_packet::<Move>(&mut &bytes[..]).unwrap(), packet);
}

#[derive(BedrockEncode, BedrockDecode, Debug, PartialEq)]
#[bedrock(tag = VarI32)]
enum Mode {
    Survival = 0,
    Creative = 1,
    Spectator = 6,
}

#[test]
fn derived_enum_writes_its_discriminant_as_the_tag() {
    assert_eq!(to_bytes(&Mode::Spectator).unwrap(), [0x0C]);
    assert_eq!(from_slice::<Mode>(&[0x02]).unwrap(), Mode::Creative);
    assert_eq!(
        from_slice::<Mode>(&[0x04]).unwrap_err(),
        Error::UnknownEnumValue(2)
    );
}
