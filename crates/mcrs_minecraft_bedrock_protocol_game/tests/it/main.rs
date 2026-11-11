use core::fmt::Debug;
use core::num::ParseIntError;

use glam::Vec3;
use mcrs_minecraft_bedrock_protocol_game::{
    BlobCache, BlockLayer, ChunkColumn, DimensionId, MoveMode, MovePlayer, PlayerAuthInput,
    SetTime, StartGame, SubChunkCount, TeleportCause, Text, TextContent, UpdateBlock,
    UpdateBlockFlags,
};
use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, NetworkBlockPos, NetworkChunkPos, Packet, VarI32, VarI64,
    VarU32, VarU64, decode_packet, encode_packet,
};

fn hex(fixture: &str) -> Result<Vec<u8>, ParseIntError> {
    fixture
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16))
        .collect()
}

fn assert_fixture<'a, P>(packet: P, bytes: &'a [u8])
where
    P: Packet + BedrockEncode + BedrockDecode<'a> + PartialEq + Debug,
{
    let mut encoded = Vec::new();
    assert_eq!(encode_packet(&packet, &mut encoded), Ok(()));
    assert_eq!(encoded, bytes, "encode {packet:?}");
    let mut rest = bytes;
    assert_eq!(
        decode_packet::<P>(&mut rest),
        Ok(packet),
        "decode {bytes:02x?}"
    );
    assert!(rest.is_empty(), "decode leaves {rest:02x?}");
}

fn text(content: TextContent<'_>) -> Text<'_> {
    Text {
        needs_translation: matches!(content, TextContent::Translation { .. }),
        content,
        sender_xuid: "",
        platform_chat_id: "",
        filtered_message: "",
    }
}

#[test]
fn text_fixtures() {
    let fixtures = [
        (
            "09 00 00 02 68 69 00 00 00",
            text(TextContent::Raw { message: "hi" }),
        ),
        (
            "09 01 00 05 53 74 65 76 65 02 68 69 00 00 00",
            text(TextContent::Chat {
                source_name: "Steve",
                message: "hi",
            }),
        ),
        (
            "09 02 01 02 25 61 01 01 78 00 00 00",
            text(TextContent::Translation {
                message: "%a",
                parameters: vec!["x"],
            }),
        ),
    ];
    for (fixture, packet) in fixtures {
        assert_fixture(packet, &hex(fixture).unwrap());
    }
    assert_eq!(
        decode_packet::<Text>(&mut &hex("09 0c 00 02 68 69 00 00 00").unwrap()[..]),
        Err(Error::UnknownEnumValue(12))
    );
}

#[test]
fn move_player_fixtures() {
    let walk = MovePlayer {
        runtime_id: VarU64(1),
        position: Vec3::new(0.5, 65.62, 0.5),
        pitch: 0.0,
        yaw: 90.0,
        head_yaw: 90.0,
        mode: MoveMode::Normal,
        on_ground: true,
        ridden_runtime_id: VarU64(0),
        tick: VarU64(100),
    };
    let teleport = MovePlayer {
        position: Vec3::new(0.0, 64.0, 0.0),
        yaw: 0.0,
        head_yaw: 0.0,
        mode: MoveMode::Teleport {
            cause: TeleportCause::Command,
            source_entity_type: 0,
        },
        on_ground: false,
        tick: VarU64(0),
        ..walk
    };
    let fixtures = [
        (
            "13 01 00 00 00 3f 71 3d 83 42 00 00 00 3f 00 00 00 00 00 00 b4 42 00 00 b4 42 00 01 00 64",
            walk,
        ),
        (
            "13 01 00 00 00 00 00 00 80 42 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00 00 03 00 00 00 00 00 00 00 00",
            teleport,
        ),
    ];
    for (fixture, packet) in fixtures {
        assert_fixture(packet, &hex(fixture).unwrap());
    }
}

#[test]
fn set_time_fixture() {
    assert_fixture(SetTime::from_world_time(6000), &hex("0a e0 5d").unwrap());
}

#[test]
fn set_time_keeps_time_of_day_and_moon_phase_outside_i32() {
    let world_time = 192_000 * 20_000 + 6000;
    assert_eq!(
        SetTime::from_world_time(world_time),
        SetTime::from_world_time(6000)
    );
}

#[test]
fn update_block_fixture() {
    let packet = UpdateBlock {
        position: NetworkBlockPos { x: 1, y: 64, z: -1 },
        block_runtime_id: VarU32(42),
        flags: UpdateBlockFlags::NEIGHBOURS | UpdateBlockFlags::NETWORK,
        layer: BlockLayer::Normal,
    };
    assert_fixture(packet, &hex("15 02 40 01 2a 03 00").unwrap());
}

#[test]
fn chunk_column_fixtures() {
    let origin = NetworkChunkPos { x: 0, z: 0 };
    let column = |position, sub_chunks, blob_cache, payload| ChunkColumn {
        position,
        dimension: DimensionId::Overworld,
        sub_chunks,
        blob_cache,
        payload,
    };
    let fixtures = [
        (
            "3a 00 00 00 ff ff ff ff 0f 00 00",
            column(
                origin,
                SubChunkCount::RequestAll,
                BlobCache::Disabled,
                &[][..],
            ),
        ),
        (
            "3a 02 01 00 fe ff ff ff 0f 04 00 00 00",
            column(
                NetworkChunkPos { x: 1, z: -1 },
                SubChunkCount::RequestUpTo { highest: 4 },
                BlobCache::Disabled,
                &[],
            ),
        ),
        (
            "3a 00 00 00 01 01 01 ef cd ab 89 67 45 23 01 01 00",
            column(
                origin,
                SubChunkCount::Included { count: 1 },
                BlobCache::Enabled {
                    blob_ids: vec![0x0123_4567_89ab_cdef],
                },
                &[0x00],
            ),
        ),
    ];
    for (fixture, packet) in fixtures {
        assert_fixture(packet, &hex(fixture).unwrap());
    }
    let request_mode_as_count = column(
        origin,
        SubChunkCount::Included { count: 0xffff_fffe },
        BlobCache::Disabled,
        &[],
    );
    assert_eq!(
        encode_packet(&request_mode_as_count, &mut Vec::new()),
        Err(Error::LengthOverflow)
    );
}

#[test]
fn start_game_round_trip() {
    let packet = StartGame {
        entity_id: VarI64(123456789),
        runtime_entity_id: VarU64(1),
        player_gamemode: VarI32(1),
        player_position: Vec3::new(100.5, 64.0, -200.5),
        pitch: 15.0,
        yaw: 90.0,
        seed: 42,
        dimension: VarI32(0),
        generator: VarI32(1),
        world_gamemode: VarI32(1),
        difficulty: VarI32(2),
        spawn_x: VarI32(100),
        spawn_y: VarU32(64),
        spawn_z: VarI32(-200),
        has_achievements_disabled: true,
        world_name: "Bedrock World".to_string(),
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = StartGame::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}

#[test]
fn player_auth_input_round_trip() {
    let packet = PlayerAuthInput {
        pitch: 10.0,
        yaw: 45.0,
        position: Vec3::new(1.0, 2.0, 3.0),
        move_vec_x: 0.0,
        move_vec_z: 1.0,
        head_yaw: 45.0,
        input_data: 0x01 | 0x04,
        input_mode: VarU32(1),
        play_mode: VarU32(0),
        tick: VarU64(120),
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = PlayerAuthInput::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}
