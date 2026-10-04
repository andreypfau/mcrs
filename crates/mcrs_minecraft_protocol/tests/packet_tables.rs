use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_protocol::packets::table::{TABLES, Table};
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
