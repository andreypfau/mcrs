use core::fmt::Debug;
use core::num::ParseIntError;

use glam::{Vec2, Vec3};
use mcrs_minecraft_bedrock_protocol_game::{
    BlobCache, BlockAction, BlockLayer, BroadcastMode, ChatRestrictionLevel, ChunkColumn,
    Difficulty, DimensionId, EditorWorldType, EducationOffer, GameRule, GameRuleValue, GameType,
    Generator, InputFlags, InputMode, InteractPrediction, InteractionModel, ItemUseAction,
    ItemUseTransaction, ItemUseTrigger, LevelSettings, MoveMode, MovePlayer, MovementAuthority,
    MovementSettings, NetworkNbt, PlayMode, PlayerAuthInput, PlayerGameType, PlayerPermission,
    SetTime, SpawnBiomeType, StartGame, SubChunkCount, TargetedBlockAction, TeleportCause, Text,
    TextContent, UpdateBlock, UpdateBlockFlags, WorldTemplateId,
};
use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, NetworkBlockPos, NetworkChunkPos, NetworkSignedBlockPos,
    Packet, VarI32, VarI64, VarU32, VarU64, decode_packet, encode_packet, from_slice, to_bytes,
};
use mcrs_minecraft_nbt::compound::NbtCompound;

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

const START_GAME: &str = "
    0b 02 01 00 00 00 00 3f 71 3d 83 42 00 00 00 3f
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 02 00 00 02 00 40 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 01 01 08 08
    01 00 01 0f 64 6f 64 61 79 6c 69 67 68 74 63 79
    63 6c 65 00 01 01 00 00 00 00 00 00 00 02 04 00
    00 00 00 00 00 00 00 00 00 00 00 00 01 2a 00 00
    00 00 00 00 00 00 01 00 00 00 00 00 00 00 00 00
    05 77 6f 72 6c 64 00 00 02 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 01 04 6d 63 72 73 0a 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00";

fn day_light_cycle() -> GameRule<'static> {
    GameRule {
        name: "dodaylightcycle",
        editable: false,
        value: GameRuleValue::Bool(true),
    }
}

#[test]
fn start_game_fixture() {
    let packet = StartGame {
        entity_unique_id: VarI64(1),
        entity_runtime_id: VarU64(1),
        player_game_type: PlayerGameType::Survival,
        position: Vec3::new(0.5, 65.62, 0.5),
        rotation: Vec2::ZERO,
        level_settings: LevelSettings {
            seed: 0,
            spawn_biome_type: SpawnBiomeType::Default,
            user_defined_biome_name: "",
            dimension: DimensionId::Overworld,
            generator: Generator::Overworld,
            world_game_type: GameType::Survival,
            hardcore: false,
            difficulty: Difficulty::Easy,
            world_spawn: NetworkBlockPos { x: 0, y: 64, z: 0 },
            achievements_disabled: false,
            editor_world_type: EditorWorldType::NotEditor,
            created_in_editor: false,
            exported_from_editor: false,
            day_cycle_stop_time: VarI32(0),
            education_offer: EducationOffer::None,
            education_features_enabled: false,
            education_product_id: "",
            rain_level: 0.0,
            lightning_level: 0.0,
            confirmed_platform_locked_content: false,
            multiplayer_game_intent: true,
            lan_broadcast_intent: true,
            xbox_live_broadcast_mode: BroadcastMode::Public,
            platform_broadcast_mode: BroadcastMode::Public,
            commands_enabled: true,
            texture_packs_required: false,
            game_rules: vec![day_light_cycle()],
            experiments: Vec::new(),
            experiments_previously_toggled: false,
            bonus_chest: false,
            start_with_map: false,
            player_permission: PlayerPermission::Member,
            server_chunk_tick_range: 4,
            locked_behaviour_pack: false,
            locked_resource_pack: false,
            from_locked_template: false,
            only_microsoft_account_gamertags: false,
            from_world_template: false,
            world_template_option_locked: false,
            only_old_villagers: false,
            persona_disabled: false,
            custom_skins_disabled: false,
            emote_chat_muted: false,
            base_game_version: "*",
            limited_world_width: 0,
            limited_world_depth: 0,
            new_nether: true,
            education_shared_resource_button_name: "",
            education_shared_resource_link: "",
            force_experimental_gameplay: None,
            chat_restriction_level: ChatRestrictionLevel::None,
            player_interactions_disabled: false,
            server_id: "",
            world_id: "",
            scenario_id: "",
        },
        level_id: "",
        level_name: "world",
        template_content_identity: "",
        trial: false,
        movement_settings: MovementSettings {
            authority: MovementAuthority::Server,
            rewind_history_size: VarI32(0),
            server_authoritative_block_breaking: false,
        },
        current_level_time: 0,
        enchantment_seed: VarI32(0),
        block_properties: Vec::new(),
        item_table: Vec::new(),
        multiplayer_correlation_id: "",
        item_stack_net_manager: true,
        server_version: "mcrs",
        player_property_data: NetworkNbt::default(),
        block_registry_checksum: 0,
        world_template_id: WorldTemplateId::default(),
        client_side_generation: false,
        block_network_ids_are_hashes: false,
        server_authoritative_sound: false,
    };
    let bytes = hex(START_GAME).unwrap();
    assert_eq!(bytes.len(), 187, "the fixture is the packet of BEG-SG-001");
    assert_fixture(packet, &bytes);
}

fn assert_codec<'a, T>(value: T, bytes: &'a [u8])
where
    T: BedrockEncode + BedrockDecode<'a> + PartialEq + Debug,
{
    assert_eq!(to_bytes(&value).as_deref(), Ok(bytes), "encode {value:?}");
    assert_eq!(from_slice::<T>(bytes), Ok(value), "decode {bytes:02x?}");
}

#[test]
fn game_rule_fixtures() {
    let rules = hex("01 0f 64 6f 64 61 79 6c 69 67 68 74 63 79 63 6c 65 00 01 01").unwrap();
    assert_eq!(to_bytes(&[day_light_cycle()][..]), Ok(rules.clone()));
    assert_eq!(
        from_slice::<Vec<GameRule>>(&rules),
        Ok(vec![day_light_cycle()])
    );
    let values = [
        ("02 05", GameRuleValue::Int(5)),
        ("03 00 00 80 3f", GameRuleValue::Float(1.0)),
    ];
    for (fixture, value) in values {
        assert_codec(value, &hex(fixture).unwrap());
    }
    assert_eq!(
        from_slice::<GameRuleValue>(&[0x04, 0x00]),
        Err(Error::UnknownEnumValue(4))
    );
}

#[test]
fn network_nbt_fixture() {
    let mut compound = NbtCompound::new();
    compound.put_int("a", -1);
    assert_codec(NetworkNbt(compound), &hex("0a 00 03 01 61 01 00").unwrap());
    let errors = [
        ("0a 00 03 01 61", Error::UnexpectedEof),
        ("0a 00 0d 01 61 00", Error::Nbt),
        ("03 00 01", Error::Nbt),
    ];
    for (bytes, error) in errors {
        assert_eq!(
            from_slice::<NetworkNbt>(&hex(bytes).unwrap()),
            Err(error),
            "{bytes}"
        );
    }
}

fn idle_input(client_tick: u64) -> PlayerAuthInput<'static> {
    PlayerAuthInput {
        pitch: 0.0,
        yaw: 0.0,
        position: Vec3::ZERO,
        move_vector: Vec2::ZERO,
        head_yaw: 0.0,
        input_flags: InputFlags::default(),
        input_mode: InputMode::Mouse,
        play_mode: PlayMode::Normal,
        interaction_model: InteractionModel::Crosshair,
        client_tick: VarU64(client_tick),
        position_delta: Vec3::ZERO,
        item_use_transaction: None,
        item_stack_request: None,
        block_actions: None,
        client_predicted_vehicle: None,
        analog_move_vector: Vec2::ZERO,
    }
}

fn walk_forward() -> PlayerAuthInput<'static> {
    PlayerAuthInput {
        yaw: 90.0,
        position: Vec3::new(0.5, 65.62, 0.5),
        move_vector: Vec2::new(0.0, 1.0),
        head_yaw: 90.0,
        input_flags: InputFlags::UP,
        analog_move_vector: Vec2::new(0.0, 1.0),
        ..idle_input(5)
    }
}

const WALK_FORWARD: &str = "90 01 00 00 00 00 00 00 b4 42 00 00 00 3f 71 3d 83 42 00 00 00 3f \
    00 00 00 00 00 00 80 3f 00 00 b4 42 80 08 01 00 01 05 00 00 00 00 00 00 00 00 00 00 00 00 \
    00 00 00 00 00 00 80 3f";

#[test]
fn player_auth_input_fixtures() {
    let start_destroy = PlayerAuthInput {
        block_actions: Some(vec![BlockAction::Targeted {
            action: TargetedBlockAction::StartDestroy,
            position: NetworkSignedBlockPos { x: 1, y: 64, z: -1 },
            face: VarI32(1),
        }]),
        ..idle_input(6)
    };
    let click_block_empty_hand = PlayerAuthInput {
        item_use_transaction: Some(ItemUseTransaction {
            legacy_request: None,
            actions: Vec::new(),
            action_type: ItemUseAction::ClickBlock,
            trigger_type: ItemUseTrigger::PlayerInput,
            block_position: NetworkBlockPos { x: 1, y: 64, z: -1 },
            block_face: VarI32(1),
            hotbar_slot: VarI32(0),
            held_item: None,
            player_position: Vec3::ZERO,
            click_position: Vec3::ZERO,
            block_runtime_id: VarU32(0),
            client_interact_prediction: InteractPrediction::Success,
        }),
        ..idle_input(7)
    };
    let zeros = |count| vec!["00"; count].join(" ");
    let fixtures = [
        (WALK_FORWARD.to_string(), walk_forward()),
        (
            format!(
                "90 01 {} 80 80 80 80 80 01 01 00 01 06 {} 02 00 02 80 01 01 02 {}",
                zeros(32),
                zeros(12),
                zeros(8)
            ),
            start_destroy,
        ),
        (
            format!(
                "90 01 {} 80 80 80 80 40 01 00 01 07 {} 00 00 00 01 02 40 01 02 00 00 {} 00 01 {}",
                zeros(32),
                zeros(12),
                zeros(24),
                zeros(8)
            ),
            click_block_empty_hand,
        ),
    ];
    for (fixture, packet) in fixtures {
        assert_fixture(packet, &hex(&fixture).unwrap());
    }
}

#[test]
fn player_auth_input_flags_follow_the_parts() {
    let block_actions_flag_without_actions = PlayerAuthInput {
        input_flags: InputFlags::from_bits(InputFlags::UP.bits() | 1 << 35),
        ..walk_forward()
    };
    let mut encoded = Vec::new();
    encode_packet(&block_actions_flag_without_actions, &mut encoded).unwrap();
    assert_eq!(encoded, hex(WALK_FORWARD).unwrap());
}
