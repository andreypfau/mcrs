use glam::Vec3;
use mcrs_minecraft_bedrock_protocol_game::{
    ChunkColumn, MoveMode, MovePlayer, PlayerAuthInput, SetTime, StartGame, TextContent,
    TextPacket, UpdateBlock,
};
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32, VarI64, VarU32, VarU64};

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
fn text_packet_round_trip() {
    let contents = [
        TextContent::Chat {
            source_name: "Steve".to_string(),
            message: "Hello Bedrock!".to_string(),
        },
        TextContent::Translate {
            message: "chat.type.text".to_string(),
            parameters: vec!["Steve".to_string(), "Hi!".to_string()],
        },
        TextContent::TextObject {
            message: r#"{"rawtext":[{"text":"Hello"}]}"#.to_string(),
        },
    ];
    for content in contents {
        let packet = TextPacket {
            content,
            needs_translation: true,
            xuid: "2535".to_string(),
            platform_chat_id: "".to_string(),
        };
        let mut buf = Vec::new();
        packet.encode(&mut buf).unwrap();

        let mut r = buf.as_slice();
        assert_eq!(TextPacket::decode(&mut r).unwrap(), packet);
        assert!(r.is_empty());
    }
}

#[test]
fn move_player_round_trip() {
    let packet = MovePlayer {
        runtime_entity_id: VarU64(10),
        position: Vec3::new(12.0, 70.0, 34.0),
        pitch: 0.0,
        yaw: 180.0,
        head_yaw: 180.0,
        mode: MoveMode::Teleport {
            cause: 2,
            source_entity_type: 63,
        },
        on_ground: true,
        riding_runtime_entity_id: VarU64(0),
        tick: 1200,
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = MovePlayer::decode(&mut r).unwrap();
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

#[test]
fn set_time_round_trip() {
    let packet = SetTime { time: VarI32(1000) };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = SetTime::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}

#[test]
fn update_block_round_trip() {
    let packet = UpdateBlock {
        x: VarI32(10),
        y: VarU32(65),
        z: VarI32(-20),
        block_runtime_id: VarU32(1337),
        flags: VarU32(0),
        layer: VarU32(0),
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = UpdateBlock::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}

#[test]
fn chunk_column_round_trip() {
    let packet = ChunkColumn {
        chunk_x: VarI32(3),
        chunk_z: VarI32(-5),
        dimension: VarI32(0),
        sub_chunk_count: VarU32(4),
        cache_enabled: false,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = ChunkColumn::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}
