use core::fmt::Debug;
use std::io;
use std::io::Write;

use flate2::Compression;
use flate2::write::{DeflateEncoder, ZlibEncoder};
use mcrs_minecraft_bedrock_protocol::{
    BatchCompression, BatchDecoder, BatchEncoder, CompressionAlgorithm, ConnectionRequest,
    DEFAULT_INFLATE_LIMIT, Disconnect, DisconnectReason, DisconnectScreen, Login, NetworkSettings,
    PlayStatus, PlayStatusKind, ProtocolError, RequestNetworkSettings, ResourcePackClientResponse,
    ResourcePackInfoEntry, ResourcePackResponse, ResourcePackStack, ResourcePacksInfo,
};
use mcrs_minecraft_bedrock_wire::{
    BeI32, BedrockDecode, BedrockEncode, Error, Experiment, Packet, decode_packet, encode_packet,
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

const PACK_ID: &str = "0fba4063-dba1-4281-9b89-ff9390653530";

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .filter_map(|byte| u8::from_str_radix(byte, 16).ok())
        .collect()
}

fn assert_fixture<'a, P>(packet: P, bytes: &'a [u8])
where
    P: Packet + BedrockEncode + BedrockDecode<'a> + PartialEq + Debug,
{
    let mut encoded = Vec::new();
    assert_eq!(encode_packet(&packet, &mut encoded), Ok(()), "{packet:?}");
    assert_eq!(encoded, bytes, "encode {packet:?}");
    let mut rest = bytes;
    assert_eq!(
        decode_packet::<P>(&mut rest),
        Ok(packet),
        "decode {bytes:02x?}"
    );
    assert!(rest.is_empty(), "decode leaves {rest:02x?}");
}

fn play_status(status: PlayStatusKind) -> PlayStatus {
    PlayStatus { status }
}

#[test]
fn handshake_fixtures() {
    assert_fixture(
        RequestNetworkSettings {
            protocol_version: BeI32(729),
        },
        &hex("c1 01 00 00 02 d9"),
    );
    assert_fixture(
        NetworkSettings {
            threshold: 512,
            algorithm: CompressionAlgorithm::Deflate,
            client_throttle_enabled: false,
            client_throttle_threshold: 0,
            client_throttle_scalar: 0.0,
        },
        &hex("8f 01 00 02 00 00 00 00 00 00 00 00"),
    );
    assert_fixture(
        Login {
            protocol_version: BeI32(729),
            connection_request: ConnectionRequest {
                chain: br#"{"chain":[]}"#,
                client_data: b"",
            },
        },
        &hex("01 00 00 02 d9 14 0c 00 00 00 7b 22 63 68 61 69 6e 22 3a 5b 5d 7d 00 00 00 00"),
    );
    assert_fixture(
        play_status(PlayStatusKind::LoginSuccess),
        &hex("02 00 00 00 00"),
    );
    assert_fixture(
        play_status(PlayStatusKind::PlayerSpawn),
        &hex("02 00 00 00 03"),
    );
    assert_fixture(
        Disconnect {
            reason: DisconnectReason::KICKED,
            screen: DisconnectScreen::Shown {
                message: "bye",
                filtered_message: "",
            },
        },
        &hex("05 6e 00 03 62 79 65 00"),
    );
    assert_fixture(
        Disconnect {
            reason: DisconnectReason::KICKED,
            screen: DisconnectScreen::Hidden,
        },
        &hex("05 6e 01"),
    );
}

#[test]
fn resource_pack_fixtures() {
    let no_packs = ResourcePacksInfo {
        packs_required: false,
        has_addon_packs: false,
        has_scripts: false,
        resource_packs: vec![],
        cdn_urls: vec![],
    };
    assert_fixture(no_packs.clone(), &hex("06 00 00 00 00 00 00"));
    assert_fixture(
        ResourcePacksInfo {
            packs_required: true,
            resource_packs: vec![ResourcePackInfoEntry {
                pack_id: PACK_ID,
                version: "1.0.0",
                size: 1024,
                content_key: "",
                sub_pack_name: "",
                content_identity: "",
                has_scripts: false,
                is_addon_pack: false,
                ray_tracing_capable: false,
            }],
            ..no_packs
        },
        &hex(
            "06 01 00 00 01 00 24 30 66 62 61 34 30 36 33 2d 64 62 61 31 2d 34 32 38 31 2d 39 62 38 39
             2d 66 66 39 33 39 30 36 35 33 35 33 30 05 31 2e 30 2e 30 00 04 00 00 00 00 00 00 00 00 00
             00 00 00 00",
        ),
    );
    let vanilla_stack = ResourcePackStack {
        packs_required: false,
        addon_packs: vec![],
        resource_packs: vec![],
        base_game_version: "*",
        experiments: vec![],
        experiments_previously_toggled: false,
        include_editor_packs: false,
    };
    assert_fixture(
        vanilla_stack.clone(),
        &hex("07 00 00 00 01 2a 00 00 00 00 00 00"),
    );
    assert_fixture(
        ResourcePackStack {
            base_game_version: "1.21.30",
            experiments: vec![Experiment {
                name: "gametest",
                enabled: true,
            }],
            experiments_previously_toggled: true,
            ..vanilla_stack
        },
        &hex("07 00 00 00 07 31 2e 32 31 2e 33 30 01 00 00 00 08 67 61 6d 65 74 65 73 74 01 01 00"),
    );
    assert_fixture(
        ResourcePackClientResponse {
            response: ResourcePackResponse::Completed,
            pack_references: vec![],
        },
        &hex("08 04 00 00"),
    );
    assert_fixture(
        ResourcePackClientResponse {
            response: ResourcePackResponse::SendPacks,
            pack_references: vec!["0fba4063-dba1-4281-9b89-ff9390653530_1.0.0"],
        },
        &hex(
            "08 02 01 00 2a 30 66 62 61 34 30 36 33 2d 64 62 61 31 2d 34 32 38 31 2d 39 62 38 39 2d 66
             66 39 33 39 30 36 35 33 35 33 30 5f 31 2e 30 2e 30",
        ),
    );
}

#[test]
fn handshake_decode_errors() {
    let login_cases: [(&str, Error); 3] = [
        (
            "01 00 00 02 d9 08 ff ff ff ff 00 00 00 00",
            Error::LengthOverflow,
        ),
        (
            "01 00 00 02 d9 08 05 00 00 00 00 00 00 00",
            Error::UnexpectedEof,
        ),
        (
            "01 00 00 02 d9 09 00 00 00 00 00 00 00 00 00",
            Error::TrailingBytes,
        ),
    ];
    for (bytes, error) in login_cases {
        let bytes = hex(bytes);
        assert_eq!(
            decode_packet::<Login>(&mut &bytes[..]).unwrap_err(),
            error,
            "{bytes:02x?}"
        );
    }
    assert_eq!(
        decode_packet::<ResourcePackClientResponse>(&mut &hex("08 00 00 00")[..]).unwrap_err(),
        Error::UnknownEnumValue(0)
    );
}

fn negotiated(threshold: u16, algorithm: CompressionAlgorithm) -> BatchCompression {
    BatchCompression::Negotiated {
        threshold,
        algorithm,
    }
}

fn switched_decoder() -> BatchDecoder {
    let mut decoder = BatchDecoder::new();
    decoder.switch();
    decoder
}

fn packets(decoder: &mut BatchDecoder, message: &[u8]) -> Result<Vec<Vec<u8>>, ProtocolError> {
    Ok(decoder.decode(message)?.map(<[u8]>::to_vec).collect())
}

#[test]
fn pending_batches_have_no_marker() {
    let mut encoder = BatchEncoder::new();
    encoder
        .append(&RequestNetworkSettings {
            protocol_version: BeI32(729),
        })
        .unwrap();
    let request = hex("fe 06 c1 01 00 00 02 d9");
    assert_eq!(encoder.finish().unwrap(), request);
    encoder
        .append(&NetworkSettings {
            threshold: 512,
            algorithm: CompressionAlgorithm::Deflate,
            client_throttle_enabled: false,
            client_throttle_threshold: 0,
            client_throttle_scalar: 0.0,
        })
        .unwrap();
    assert_eq!(
        encoder.finish().unwrap(),
        hex("fe 0c 8f 01 00 02 00 00 00 00 00 00 00 00")
    );

    let mut decoder = BatchDecoder::new();
    assert_eq!(
        packets(&mut decoder, &request).unwrap(),
        [hex("c1 01 00 00 02 d9")]
    );
}

#[test]
fn marker_follows_threshold_and_algorithm() {
    let one = hex("02 00 00 00 00");
    let uncompressed = hex("fe ff 05 02 00 00 00 00");
    let deflated = hex("fe 00 63 65 62 00 02 00");
    let snappy = hex("fe 01 06 14 05 02 00 00 00 00");
    let cases = [
        (negotiated(1, CompressionAlgorithm::Deflate), &deflated),
        (negotiated(6, CompressionAlgorithm::Deflate), &deflated),
        (negotiated(7, CompressionAlgorithm::Deflate), &uncompressed),
        (negotiated(0, CompressionAlgorithm::Deflate), &uncompressed),
        (negotiated(1, CompressionAlgorithm::Disabled), &uncompressed),
        (negotiated(7, CompressionAlgorithm::Snappy), &uncompressed),
        (negotiated(1, CompressionAlgorithm::Snappy), &snappy),
    ];
    let mut encoder = BatchEncoder::new();
    let mut decoder = switched_decoder();
    for (compression, expected) in cases {
        encoder.set_compression(compression);
        encoder
            .append(&play_status(PlayStatusKind::LoginSuccess))
            .unwrap();
        assert_eq!(encoder.finish().unwrap(), expected, "{compression:?}");
        assert_eq!(packets(&mut decoder, expected).unwrap(), [one.as_slice()]);
    }
}

#[test]
fn packet_list_carries_packets_in_order() {
    let mut encoder = BatchEncoder::new();
    encoder.set_compression(negotiated(0, CompressionAlgorithm::Deflate));
    encoder
        .append(&play_status(PlayStatusKind::LoginSuccess))
        .unwrap();
    encoder
        .append(&play_status(PlayStatusKind::PlayerSpawn))
        .unwrap();
    let message = encoder.finish().unwrap().to_vec();
    assert_eq!(message, hex("fe ff 05 02 00 00 00 00 05 02 00 00 00 03"));
    assert_eq!(
        packets(&mut switched_decoder(), &message).unwrap(),
        [hex("02 00 00 00 00"), hex("02 00 00 00 03")]
    );
}

fn deflate_batch(packet_list: &[u8]) -> io::Result<Vec<u8>> {
    let mut deflate = DeflateEncoder::new(vec![0xfe, 0x00], Compression::default());
    deflate.write_all(packet_list)?;
    deflate.finish()
}

#[test]
fn batch_errors() {
    let one = hex("05 02 00 00 00 00");
    let mut zlib = ZlibEncoder::new(vec![0xfe, 0x00], Compression::default());
    zlib.write_all(&one).unwrap();
    let mut trailing = deflate_batch(&one).unwrap();
    trailing.push(0x00);
    let mut truncated = deflate_batch(&one).unwrap();
    truncated.pop();
    let cases: [(Vec<u8>, ProtocolError); 12] = [
        (hex("ff 05 02 00 00 00 00"), ProtocolError::NotABatch(0xff)),
        (hex("fe"), ProtocolError::MissingMarker),
        (
            hex("fe 02 05 02 00 00 00 00"),
            ProtocolError::UnknownMarker(0x02),
        ),
        (hex("fe 01 06 14 05 02"), ProtocolError::CorruptSnappy),
        (
            hex("fe 01 ff ff ff ff 0f 00"),
            ProtocolError::InflateLimitExceeded {
                limit: DEFAULT_INFLATE_LIMIT,
            },
        ),
        (hex("fe ff"), ProtocolError::EmptyPacketList),
        (hex("fe ff 05 02 00 00 00 00 00"), ProtocolError::EmptyEntry),
        (
            hex("fe ff 05 02 00 00 00 00 05 02"),
            ProtocolError::TruncatedEntry,
        ),
        (zlib.finish().unwrap(), ProtocolError::CorruptDeflate),
        (trailing, ProtocolError::CorruptDeflate),
        (truncated, ProtocolError::CorruptDeflate),
        (deflate_batch(&[]).unwrap(), ProtocolError::EmptyPacketList),
    ];
    let mut decoder = switched_decoder();
    for (message, error) in cases {
        assert_eq!(
            packets(&mut decoder, &message).unwrap_err(),
            error,
            "{message:02x?}"
        );
    }
    assert!(packets(&mut decoder, &[]).unwrap().is_empty());
}

#[test]
fn inflate_limit_accepts_exactly_the_limit() {
    let mut packet_list = Vec::new();
    for _ in 0..1000 {
        packet_list.extend(hex("05 02 00 00 00 00"));
    }
    let message = deflate_batch(&packet_list).unwrap();

    let mut at_limit = BatchDecoder::new_with_inflate_limit(packet_list.len());
    at_limit.switch();
    assert_eq!(at_limit.decode(&message).unwrap().count(), 1000);

    let mut below = BatchDecoder::new_with_inflate_limit(packet_list.len() - 1);
    below.switch();
    assert_eq!(
        below.decode(&message).unwrap_err(),
        ProtocolError::InflateLimitExceeded {
            limit: packet_list.len() - 1
        }
    );
}

#[test]
fn steady_state_cycle_does_not_allocate() {
    for algorithm in [CompressionAlgorithm::Deflate, CompressionAlgorithm::Snappy] {
        let mut encoder = BatchEncoder::new();
        encoder.set_compression(negotiated(1, algorithm));
        let mut decoder = switched_decoder();
        let mut cycle = || {
            for _ in 0..64 {
                encoder
                    .append(&play_status(PlayStatusKind::PlayerSpawn))
                    .unwrap();
            }
            let message = encoder.finish().unwrap();
            let mut count = 0;
            for mut packet in decoder.decode(message).unwrap() {
                let status = decode_packet::<PlayStatus>(&mut packet).unwrap();
                assert_eq!(status.status, PlayStatusKind::PlayerSpawn);
                count += 1;
            }
            count
        };
        assert_eq!(cycle(), 64);
        let (allocated, count) = counting_allocator::allocated_by(&mut cycle);
        assert_eq!(count, 64);
        assert_eq!(allocated, 0, "a warm {algorithm:?} cycle allocates");
    }
}
