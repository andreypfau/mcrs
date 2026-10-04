use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_protocol::packets::table::{Decoded, TABLES, Table, game_serverbound};
use mcrs_minecraft_protocol::packets::{configuration, game, intent, login, status};
use mcrs_minecraft_protocol::{ConnectionState, Packet, PacketSide};
use serde::Deserialize;

#[derive(Deserialize)]
struct Row {
    protocol_id: i32,
}

type Report = BTreeMap<String, BTreeMap<String, BTreeMap<String, Row>>>;

fn report() -> Report {
    serde_json::from_str(include_str!("../../../assets/mcrs/reports/packets.json")).unwrap()
}

fn state_key(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Handshaking => "handshake",
        ConnectionState::Status => "status",
        ConnectionState::Login => "login",
        ConnectionState::Configuration => "configuration",
        ConnectionState::Game => "play",
    }
}

fn side_key(side: PacketSide) -> &'static str {
    match side {
        PacketSide::Clientbound => "clientbound",
        PacketSide::Serverbound => "serverbound",
    }
}

fn table_of(state: ConnectionState, side: PacketSide) -> &'static Table {
    TABLES
        .iter()
        .find(|table| table.state == state && table.side == side)
        .unwrap_or_else(|| panic!("no table for {state:?} {side:?}"))
}

#[test]
fn each_table_equals_the_report_in_names_and_order() {
    let report = report();
    for table in TABLES {
        let rows = &report[state_key(table.state)][side_key(table.side)];
        let mut by_id: Vec<(i32, &str)> = rows
            .iter()
            .map(|(name, row)| (row.protocol_id, name.strip_prefix("minecraft:").unwrap()))
            .collect();
        by_id.sort();
        let ids: Vec<i32> = by_id.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            (0..by_id.len() as i32).collect::<Vec<_>>(),
            "{} ids in the report",
            table.name
        );
        let names: Vec<&str> = by_id.iter().map(|(_, name)| *name).collect();
        assert_eq!(names, table.names, "{}", table.name);
    }
}

#[test]
fn the_tables_are_the_reports_tables() {
    let from_report: BTreeSet<(String, String)> = report()
        .into_iter()
        .flat_map(|(state, sides)| sides.into_keys().map(move |side| (state.clone(), side)))
        .collect();
    let mut from_tables: Vec<(String, String)> = TABLES
        .iter()
        .map(|table| {
            (
                state_key(table.state).to_owned(),
                side_key(table.side).to_owned(),
            )
        })
        .collect();
    from_tables.sort();
    assert_eq!(from_tables, from_report.into_iter().collect::<Vec<_>>());
}

#[test]
fn the_bundle_delimiter_is_clientbound_game_row_zero() {
    let table = table_of(ConnectionState::Game, PacketSide::Clientbound);
    assert_eq!(table.names[0], "bundle_delimiter");
}

#[test]
fn names_are_unique_inside_a_table() {
    for table in TABLES {
        let distinct: BTreeSet<&str> = table.names.iter().copied().collect();
        assert_eq!(distinct.len(), table.names.len(), "{}", table.name);
    }
}

type Seen = BTreeMap<&'static str, BTreeSet<&'static str>>;

fn walk<P: Packet>(seen: &mut Seen) {
    let table = table_of(P::STATE, P::SIDE);
    assert!(
        table.typed.contains(&P::NAME),
        "{} is not a typed row of {}",
        P::NAME,
        table.name
    );
    let row = usize::try_from(P::ID).unwrap();
    assert_eq!(table.names.get(row), Some(&P::NAME), "{}", table.name);
    seen.entry(table.name).or_default().insert(P::NAME);
}

macro_rules! structures {
    ($($ty:ty,)*) => {{
        let mut seen = Seen::new();
        $(walk::<$ty>(&mut seen);)*
        seen
    }};
}

fn structures() -> Seen {
    structures! {
        intent::serverbound::ServerboundHandshake<'_>,
        status::clientbound::StatusResponse<'_>,
        status::serverbound::StatusRequest,
        status::clientbound::PongResponse,
        status::serverbound::PingRequest,
        login::clientbound::ClientboundLoginDisconnect<'_>,
        login::clientbound::ClientboundHello<'_>,
        login::clientbound::ClientboundLoginFinished<'_>,
        login::clientbound::LoginCompression,
        login::serverbound::ServerboundHello<'_>,
        login::serverbound::ServerboundKey<'_>,
        login::serverbound::ServerboundCustomQueryAnswer<'_>,
        login::serverbound::ServerboundLoginAcknowledged,
        login::serverbound::ServerboundCookieResponse<'_>,
        configuration::clientbound::ClientboundDisconnect,
        configuration::clientbound::ClientboundFinishConfiguration,
        configuration::clientbound::ClientboundKeepAlive,
        configuration::clientbound::ClientboundRegistryData<'_>,
        configuration::clientbound::ClientboundSelectKnownPacks<'_>,
        configuration::clientbound::ClientboundShowDialog,
        configuration::clientbound::ClientboundUpdateTags<'_>,
        configuration::serverbound::ServerboundClientInformation<'_>,
        configuration::serverbound::ServerboundCookieResponse<'_>,
        configuration::serverbound::ServerboundCustomPayload<'_>,
        configuration::serverbound::ServerboundFinishConfiguration,
        configuration::serverbound::ServerboundKeepAlive,
        configuration::serverbound::ServerboundPong,
        configuration::serverbound::ServerboundResourcePack,
        configuration::serverbound::ServerboundSelectKnownPacks<'_>,
        configuration::serverbound::ServerboundCustomClickAction<'_>,
        configuration::serverbound::ServerboundAcceptCodeOfConduct,
        game::clientbound::ClientboundAddEntity,
        game::clientbound::ClientboundBlockDestruction,
        game::clientbound::ClientboundBlockUpdate,
        game::clientbound::ClientboundChunkBatchFinished,
        game::clientbound::ClientboundChunkBatchStart,
        game::clientbound::ClientboundContainerClose,
        game::clientbound::ClientboundContainerSetContent,
        game::clientbound::ClientboundContainerSetData,
        game::clientbound::ClientboundContainerSetSlot,
        game::clientbound::ClientboundDisconnect,
        game::clientbound::ClientboundEntityEvent,
        game::clientbound::ClientboundEntityPositionSync,
        game::clientbound::ClientboundForgetLevelChunk,
        game::clientbound::ClientboundGameEvent,
        game::clientbound::ClientboundKeepAlive,
        game::clientbound::ClientboundLevelChunkWithLight<'_>,
        game::clientbound::ClientboundLevelParticles,
        game::clientbound::ClientboundLightUpdate<'_>,
        game::clientbound::ClientboundLogin<'_>,
        game::clientbound::ClientboundMoveEntityPos,
        game::clientbound::ClientboundMoveEntityPosRot,
        game::clientbound::ClientboundMerchantOffers,
        game::clientbound::ClientboundMoveMinecartAlongTrack,
        game::clientbound::ClientboundMoveEntityRot,
        game::clientbound::ClientboundOpenScreen,
        game::clientbound::ClientboundPlayerInfoUpdate<'_>,
        game::clientbound::ClientboundPlayerPosition,
        game::clientbound::ClientboundRecipeBookAdd,
        game::clientbound::ClientboundRecipeBookRemove,
        game::clientbound::ClientboundRecipeBookSettings,
        game::clientbound::ClientboundRemoveEntities,
        game::clientbound::ClientboundRespawn<'_>,
        game::clientbound::ClientboundRotateHead,
        game::clientbound::ClientboundSectionBlocksUpdate<'_>,
        game::clientbound::ClientboundSetCursorItem,
        game::clientbound::ClientboundSetEntityData<'_>,
        game::clientbound::ClientboundSetEquipment,
        game::clientbound::ClientboundSetHeldSlot,
        game::clientbound::ClientboundSetPassengers,
        game::clientbound::ClientboundSetPlayerInventory,
        game::clientbound::ClientboundTakeItemEntity,
        game::clientbound::ClientboundUpdateAdvancements,
        game::clientbound::ClientboundUpdateAttributes<'_>,
        game::clientbound::ClientboundUpdateRecipes,
        game::clientbound::ClientboundSetChunkCacheCenter,
        game::clientbound::ClientboundChunkCacheRadius,
        game::clientbound::ClientboundStartConfiguration,
        game::clientbound::ClientboundSystemChatPacket,
        game::serverbound::ServerboundAcceptTeleportation,
        game::serverbound::ServerboundBlockEntityTagQuery,
        game::serverbound::ServerboundSelectBundleItem,
        game::serverbound::ServerboundChangeDifficulty,
        game::serverbound::ServerboundChangeGameMode,
        game::serverbound::ServerboundChatAck,
        game::serverbound::ServerboundChatCommand<'_>,
        game::serverbound::ServerboundChatCommandSigned<'_>,
        game::serverbound::ServerboundChat<'_>,
        game::serverbound::ServerboundChatSessionUpdate,
        game::serverbound::ServerboundChunkBatchReceived,
        game::serverbound::ServerboundClientInformation<'_>,
        game::serverbound::ServerboundConfigurationAcknowledged,
        game::serverbound::ServerboundContainerButtonClick,
        game::serverbound::ServerboundContainerClick,
        game::serverbound::ServerboundContainerClose,
        game::serverbound::ServerboundContainerSlotStateChanged,
        game::serverbound::ServerboundEditBook<'_>,
        game::serverbound::ServerboundKeepAlive,
        game::serverbound::ServerboundMovePlayerPos,
        game::serverbound::ServerboundMovePlayerPosRot,
        game::serverbound::ServerboundMovePlayerRot,
        game::serverbound::ServerboundMovePlayerStatusOnly,
        game::serverbound::ServerboundPickItemFromBlock,
        game::serverbound::ServerboundPickItemFromEntity,
        game::serverbound::ServerboundPlaceRecipe,
        game::serverbound::ServerboundPlayerAction,
        game::serverbound::ServerboundRenameItem<'_>,
        game::serverbound::ServerboundSelectTrade,
        game::serverbound::ServerboundRecipeBookChangeSettings,
        game::serverbound::ServerboundRecipeBookSeenRecipe,
        game::serverbound::ServerboundSeenAdvancements,
        game::serverbound::ServerboundSetCarriedItem,
        game::serverbound::ServerboundSetCreativeModeSlot,
        game::serverbound::ServerboundUseItemOn,
        game::serverbound::ServerboundAttack,
        game::serverbound::ServerboundClientCommand,
        game::serverbound::ServerboundClientTickEnd,
        game::serverbound::ServerboundInteract,
        game::serverbound::ServerboundMoveVehicle,
        game::serverbound::ServerboundPlayerAbilities,
        game::serverbound::ServerboundPlayerCommand,
        game::serverbound::ServerboundPlayerInput,
        game::serverbound::ServerboundPlayerLoaded,
        game::serverbound::ServerboundPunch,
        game::serverbound::ServerboundUseItem,
        game::serverbound::ServerboundPong,
        game::serverbound::ServerboundResourcePack,
        game::serverbound::ServerboundCookieResponse<'_>,
        game::serverbound::ServerboundCustomPayload<'_>,
        game::serverbound::ServerboundCustomClickAction<'_>,
    }
}

#[test]
fn a_typed_row_has_the_position_of_its_name_as_its_id() {
    structures();
    assert_eq!(
        <intent::serverbound::ServerboundHandshake<'_> as Packet>::NAME,
        "intention"
    );
}

#[test]
fn the_typed_rows_are_the_rows_with_a_structure() {
    let seen = structures();
    for table in TABLES {
        let positions: Vec<usize> = table
            .typed
            .iter()
            .map(|name| table.names.iter().position(|row| row == name).unwrap())
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{} lists its typed rows out of order",
            table.name
        );
        let typed: BTreeSet<&str> = table.typed.iter().copied().collect();
        assert_eq!(
            typed,
            seen.get(table.name).cloned().unwrap_or_default(),
            "{}",
            table.name
        );
    }
}

#[test]
fn the_rows_carry_the_report_names_where_the_type_name_differs() {
    assert_eq!(
        <game::serverbound::ServerboundSelectBundleItem as Packet>::NAME,
        "bundle_item_selected"
    );
    assert_eq!(
        <game::clientbound::ClientboundChunkCacheRadius as Packet>::NAME,
        "set_chunk_cache_radius"
    );
    assert_eq!(
        <game::clientbound::ClientboundSystemChatPacket as Packet>::NAME,
        "system_chat"
    );
}

macro_rules! collect_tables {
    ($($table:ident: $enum:ident $(<$tlt:lifetime>)?, $state:ident, $side:ident in ($($module:tt)*) {
        $($name:literal $(=> $ty:ident $(<$lt:lifetime>)?)?,)*
    })*) => {
        fn expanded() -> Vec<(&'static str, Vec<&'static str>)> {
            vec![$((stringify!($table), vec![$($name),*])),*]
        }
    };
}

mcrs_minecraft_protocol::for_each_packet_table!(collect_tables);

#[test]
fn another_crate_expands_the_row_list() {
    let expanded = expanded();
    assert_eq!(expanded.len(), TABLES.len());
    for ((ident, names), table) in expanded.iter().zip(TABLES) {
        assert_eq!(*ident, table.name);
        assert_eq!(names.as_slice(), table.names, "{ident}");
    }
}

#[derive(Debug)]
enum Outcome {
    Packet,
    NotImplemented { name: &'static str, left: usize },
    Unknown(i32),
    Failed(String),
}

fn outcome<P>(result: anyhow::Result<Decoded<P>>, left: usize) -> Outcome {
    match result {
        Ok(Decoded::Packet(_)) => Outcome::Packet,
        Ok(Decoded::NotImplemented(name)) => Outcome::NotImplemented { name, left },
        Ok(Decoded::Unknown(id)) => Outcome::Unknown(id),
        Err(error) => Outcome::Failed(format!("{error:#}")),
    }
}

macro_rules! decode_each_id {
    ($($table:ident: $enum:ident $(<$tlt:lifetime>)?, $state:ident, $side:ident in ($($module:tt)*) {
        $($name:literal $(=> $ty:ident $(<$lt:lifetime>)?)?,)*
    })*) => {
        fn decode_each_id(body: &[u8]) -> Vec<(&'static str, i32, Outcome)> {
            let mut outcomes = Vec::new();
            $(
                for id in -1..=mcrs_minecraft_protocol::packets::table::$table::NAMES.len() as i32 {
                    let mut r = body;
                    let result = mcrs_minecraft_protocol::packets::table::$table::decode(id, &mut r);
                    outcomes.push((stringify!($table), id, outcome(result, r.len())));
                }
            )*
            outcomes
        }
    };
}

mcrs_minecraft_protocol::for_each_packet_table!(decode_each_id);

fn table_named(name: &str) -> &'static Table {
    TABLES.iter().find(|table| table.name == name).unwrap()
}

#[test]
fn a_typed_id_decodes_to_its_packet() {
    let table = table_named("game_serverbound");
    let id = table.names.iter().position(|n| *n == "keep_alive").unwrap() as i32;
    let body = 7i64.to_be_bytes();
    let mut r = &body[..];
    match game_serverbound::decode(id, &mut r).unwrap() {
        Decoded::Packet(game_serverbound::ServerboundGamePacket::ServerboundKeepAlive(alive)) => {
            assert_eq!(alive.0.payload, 7)
        }
        other => panic!("not the keep alive packet: {other:?}"),
    }
    assert!(r.is_empty());
}

#[test]
fn a_named_row_is_known_and_its_body_is_discarded() {
    let table = table_named("game_serverbound");
    let row = table
        .names
        .iter()
        .position(|name| !table.typed.contains(name))
        .expect("a game_serverbound row without a structure");
    let body = [1u8, 2, 3, 4];
    let mut r = &body[..];
    match game_serverbound::decode(row as i32, &mut r).unwrap() {
        Decoded::NotImplemented(name) => assert_eq!(name, table.names[row]),
        other => panic!("not a known but unimplemented row: {other:?}"),
    }
    assert!(r.is_empty());
}

#[test]
fn an_id_outside_the_table_is_unknown() {
    let rows = table_named("game_serverbound").names.len() as i32;
    for id in [-1, rows, rows + 1, i32::MAX, i32::MIN] {
        let body = [9u8, 9];
        let mut r = &body[..];
        match game_serverbound::decode(id, &mut r).unwrap() {
            Decoded::Unknown(unknown) => assert_eq!(unknown, id),
            other => panic!("id {id} is not unknown: {other:?}"),
        }
        assert_eq!(r.len(), 2, "an unknown id leaves the body alone");
    }
}

#[test]
fn every_row_of_every_table_has_an_outcome() {
    let body = [0u8; 3];
    for (table, id, outcome) in decode_each_id(&body) {
        let names = table_named(table).names;
        let typed = table_named(table).typed;
        let in_table = usize::try_from(id).ok().filter(|row| *row < names.len());
        match (in_table, &outcome) {
            (None, Outcome::Unknown(unknown)) => assert_eq!(*unknown, id, "{table}"),
            (None, other) => panic!("{table} id {id} is outside the table but is {other:?}"),
            (Some(row), Outcome::NotImplemented { name, left }) => {
                assert_eq!(*name, names[row], "{table}");
                assert!(!typed.contains(name), "{table} {name} has a structure");
                assert_eq!(*left, 0, "{table} {name} leaves its body");
            }
            (Some(row), Outcome::Packet) => {
                assert!(typed.contains(&names[row]), "{table} {}", names[row]);
            }
            (Some(row), Outcome::Failed(error)) => {
                assert!(typed.contains(&names[row]), "{table} {}", names[row]);
                assert!(error.contains(names[row]), "{table}: {error}");
            }
            (Some(_), Outcome::Unknown(_)) => panic!("{table} id {id} is a row but is unknown"),
        }
    }
}

#[test]
fn a_truncated_typed_body_is_an_error_naming_the_packet() {
    let table = table_named("game_serverbound");
    let id = table.names.iter().position(|n| *n == "keep_alive").unwrap() as i32;
    let body = 7i64.to_be_bytes();
    let mut r = &body[..3];
    let error = game_serverbound::decode(id, &mut r).unwrap_err();
    assert!(format!("{error:#}").contains("keep_alive"), "{error:#}");
}

#[test]
fn no_body_makes_a_decode_panic() {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut bodies: Vec<Vec<u8>> = vec![vec![0xff; 64], vec![0x00; 64], vec![0x7f; 64]];
    for length in 0..40usize {
        for _ in 0..16 {
            bodies.push((0..length).map(|_| next() as u8).collect());
        }
    }
    for body in &bodies {
        decode_each_id(body);
    }
}

#[test]
fn a_table_header_names_its_enum_after_its_side_and_state() {
    for table in TABLES {
        let state = match table.state {
            ConnectionState::Handshaking => "Handshake".to_owned(),
            other => format!("{other:?}"),
        };
        assert_eq!(
            table.enum_name,
            format!("{:?}{state}Packet", table.side),
            "{}",
            table.name
        );
    }
}

#[test]
fn a_deeply_nested_click_action_payload_is_refused_and_does_not_overflow() {
    let table = table_named("game_serverbound");
    let id = table
        .names
        .iter()
        .position(|n| *n == "custom_click_action")
        .unwrap() as i32;
    let mut tag = vec![9u8];
    for _ in 0..4000 {
        tag.extend_from_slice(&[9, 0, 0, 0, 1]);
    }
    tag.extend_from_slice(&[0, 0, 0, 0, 0]);
    assert!(tag.len() < 65536);
    let mut body = vec![3u8];
    body.extend_from_slice(b"a:b");
    body.extend_from_slice(&[(tag.len() & 0x7f) as u8 | 0x80, (tag.len() >> 7) as u8]);
    body.extend_from_slice(&tag);
    let mut r = &body[..];
    let error = game_serverbound::decode(id, &mut r).unwrap_err();
    assert!(
        format!("{error:#}").contains("custom_click_action"),
        "{error:#}"
    );
}
