use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_protocol::packets::table::TABLES;
use mcrs_minecraft_protocol::packets::{configuration, game, intent, login, status};
use mcrs_minecraft_protocol::{ConnectionState, Packet, PacketSide};
use serde::Deserialize;

#[derive(Deserialize)]
struct Row {
    protocol_id: i32,
}

type Report = BTreeMap<String, BTreeMap<String, BTreeMap<String, Row>>>;

type Pins = Vec<(ConnectionState, PacketSide, &'static str)>;

fn report() -> Report {
    serde_json::from_str(include_str!("../../../../assets/mcrs/reports/packets.json")).unwrap()
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

fn assert_packet<P: Packet>(
    report: &Report,
    name: &'static str,
    side: PacketSide,
    state: ConnectionState,
) {
    assert_eq!(P::NAME, name, "a structure is pinned to another name");
    assert_eq!(P::SIDE, side, "{name} has the wrong side");
    assert_eq!(P::STATE, state, "{name} has the wrong state");
    let row = report
        .get(state_key(state))
        .and_then(|sides| sides.get(side_key(side)))
        .and_then(|rows| rows.get(&format!("minecraft:{name}")))
        .unwrap_or_else(|| panic!("the report has no {state:?} {side:?} packet {name}"));
    assert_eq!(
        P::ID,
        row.protocol_id,
        "{state:?} {side:?} {name} has the wrong id"
    );
}

macro_rules! pinned {
    ($($name:literal => $ty:ty, $side:ident, $state:ident;)*) => {{
        let report = report();
        let mut pins = Pins::new();
        $(
            assert_packet::<$ty>(&report, $name, PacketSide::$side, ConnectionState::$state);
            pins.push((ConnectionState::$state, PacketSide::$side, $name));
        )*
        pins
    }};
}

fn handshake_and_status() -> Pins {
    pinned! {
        "intention" => intent::serverbound::ServerboundHandshake<'_>, Serverbound, Handshaking;
        "status_request" => status::serverbound::StatusRequest, Serverbound, Status;
        "ping_request" => status::serverbound::PingRequest, Serverbound, Status;
        "status_response" => status::clientbound::StatusResponse<'_>, Clientbound, Status;
        "pong_response" => status::clientbound::PongResponse, Clientbound, Status;
    }
}

fn login() -> Pins {
    pinned! {
        "login_disconnect" => login::clientbound::ClientboundLoginDisconnect<'_>, Clientbound, Login;
        "hello" => login::clientbound::ClientboundHello<'_>, Clientbound, Login;
        "login_finished" => login::clientbound::ClientboundLoginFinished<'_>, Clientbound, Login;
        "login_compression" => login::clientbound::LoginCompression, Clientbound, Login;

        "hello" => login::serverbound::ServerboundHello<'_>, Serverbound, Login;
        "key" => login::serverbound::ServerboundKey<'_>, Serverbound, Login;
        "custom_query_answer" => login::serverbound::ServerboundCustomQueryAnswer<'_>, Serverbound, Login;
        "login_acknowledged" => login::serverbound::ServerboundLoginAcknowledged, Serverbound, Login;
        "cookie_response" => login::serverbound::ServerboundCookieResponse<'_>, Serverbound, Login;
    }
}

fn configuration() -> Pins {
    pinned! {
        "disconnect" => configuration::clientbound::ClientboundDisconnect, Clientbound, Configuration;
        "finish_configuration" => configuration::clientbound::ClientboundFinishConfiguration, Clientbound, Configuration;
        "keep_alive" => configuration::clientbound::ClientboundKeepAlive, Clientbound, Configuration;
        "registry_data" => configuration::clientbound::ClientboundRegistryData<'_>, Clientbound, Configuration;
        "select_known_packs" => configuration::clientbound::ClientboundSelectKnownPacks<'_>, Clientbound, Configuration;
        "show_dialog" => configuration::clientbound::ClientboundShowDialog, Clientbound, Configuration;
        "update_tags" => configuration::clientbound::ClientboundUpdateTags<'_>, Clientbound, Configuration;

        "client_information" => configuration::serverbound::ServerboundClientInformation<'_>, Serverbound, Configuration;
        "cookie_response" => configuration::serverbound::ServerboundCookieResponse<'_>, Serverbound, Configuration;
        "custom_payload" => configuration::serverbound::ServerboundCustomPayload<'_>, Serverbound, Configuration;
        "finish_configuration" => configuration::serverbound::ServerboundFinishConfiguration, Serverbound, Configuration;
        "keep_alive" => configuration::serverbound::ServerboundKeepAlive, Serverbound, Configuration;
        "pong" => configuration::serverbound::ServerboundPong, Serverbound, Configuration;
        "resource_pack" => configuration::serverbound::ServerboundResourcePack, Serverbound, Configuration;
        "select_known_packs" => configuration::serverbound::ServerboundSelectKnownPacks<'_>, Serverbound, Configuration;
        "custom_click_action" => configuration::serverbound::ServerboundCustomClickAction<'_>, Serverbound, Configuration;
        "accept_code_of_conduct" => configuration::serverbound::ServerboundAcceptCodeOfConduct, Serverbound, Configuration;
    }
}

fn play_clientbound() -> Pins {
    use game::clientbound::*;
    pinned! {
        "add_entity" => ClientboundAddEntity, Clientbound, Game;
        "block_destruction" => ClientboundBlockDestruction, Clientbound, Game;
        "block_update" => ClientboundBlockUpdate, Clientbound, Game;
        "chunk_batch_finished" => ClientboundChunkBatchFinished, Clientbound, Game;
        "chunk_batch_start" => ClientboundChunkBatchStart, Clientbound, Game;
        "container_close" => ClientboundContainerClose, Clientbound, Game;
        "container_set_content" => ClientboundContainerSetContent, Clientbound, Game;
        "container_set_data" => ClientboundContainerSetData, Clientbound, Game;
        "container_set_slot" => ClientboundContainerSetSlot, Clientbound, Game;
        "disconnect" => ClientboundDisconnect, Clientbound, Game;
        "entity_event" => ClientboundEntityEvent, Clientbound, Game;
        "entity_position_sync" => ClientboundEntityPositionSync, Clientbound, Game;
        "forget_level_chunk" => ClientboundForgetLevelChunk, Clientbound, Game;
        "game_event" => ClientboundGameEvent, Clientbound, Game;
        "keep_alive" => ClientboundKeepAlive, Clientbound, Game;
        "level_chunk_with_light" => ClientboundLevelChunkWithLight<'_>, Clientbound, Game;
        "level_particles" => ClientboundLevelParticles, Clientbound, Game;
        "light_update" => ClientboundLightUpdate<'_>, Clientbound, Game;
        "login" => ClientboundLogin<'_>, Clientbound, Game;
        "move_entity_pos" => ClientboundMoveEntityPos, Clientbound, Game;
        "move_entity_pos_rot" => ClientboundMoveEntityPosRot, Clientbound, Game;
        "merchant_offers" => ClientboundMerchantOffers, Clientbound, Game;
        "move_minecart_along_track" => ClientboundMoveMinecartAlongTrack, Clientbound, Game;
        "move_entity_rot" => ClientboundMoveEntityRot, Clientbound, Game;
        "open_screen" => ClientboundOpenScreen, Clientbound, Game;
        "player_info_update" => ClientboundPlayerInfoUpdate<'_>, Clientbound, Game;
        "player_position" => ClientboundPlayerPosition, Clientbound, Game;
        "recipe_book_add" => ClientboundRecipeBookAdd, Clientbound, Game;
        "recipe_book_remove" => ClientboundRecipeBookRemove, Clientbound, Game;
        "recipe_book_settings" => ClientboundRecipeBookSettings, Clientbound, Game;
        "remove_entities" => ClientboundRemoveEntities, Clientbound, Game;
        "respawn" => ClientboundRespawn<'_>, Clientbound, Game;
        "rotate_head" => ClientboundRotateHead, Clientbound, Game;
        "section_blocks_update" => ClientboundSectionBlocksUpdate<'_>, Clientbound, Game;
        "set_cursor_item" => ClientboundSetCursorItem, Clientbound, Game;
        "set_entity_data" => ClientboundSetEntityData<'_>, Clientbound, Game;
        "set_equipment" => ClientboundSetEquipment, Clientbound, Game;
        "set_held_slot" => ClientboundSetHeldSlot, Clientbound, Game;
        "set_passengers" => ClientboundSetPassengers, Clientbound, Game;
        "set_player_inventory" => ClientboundSetPlayerInventory, Clientbound, Game;
        "take_item_entity" => ClientboundTakeItemEntity, Clientbound, Game;
        "update_advancements" => ClientboundUpdateAdvancements, Clientbound, Game;
        "update_attributes" => ClientboundUpdateAttributes<'_>, Clientbound, Game;
        "update_recipes" => ClientboundUpdateRecipes, Clientbound, Game;
        "set_chunk_cache_center" => ClientboundSetChunkCacheCenter, Clientbound, Game;
        "set_chunk_cache_radius" => ClientboundChunkCacheRadius, Clientbound, Game;
        "start_configuration" => ClientboundStartConfiguration, Clientbound, Game;
        "system_chat" => ClientboundSystemChatPacket, Clientbound, Game;
    }
}

fn play_serverbound() -> Pins {
    use game::serverbound::*;
    pinned! {
        "accept_teleportation" => ServerboundAcceptTeleportation, Serverbound, Game;
        "attack" => ServerboundAttack, Serverbound, Game;
        "client_command" => ServerboundClientCommand, Serverbound, Game;
        "client_tick_end" => ServerboundClientTickEnd, Serverbound, Game;
        "interact" => ServerboundInteract, Serverbound, Game;
        "move_vehicle" => ServerboundMoveVehicle, Serverbound, Game;
        "player_abilities" => ServerboundPlayerAbilities, Serverbound, Game;
        "player_command" => ServerboundPlayerCommand, Serverbound, Game;
        "player_input" => ServerboundPlayerInput, Serverbound, Game;
        "player_loaded" => ServerboundPlayerLoaded, Serverbound, Game;
        "punch" => ServerboundPunch, Serverbound, Game;
        "use_item" => ServerboundUseItem, Serverbound, Game;
        "cookie_response" => ServerboundCookieResponse<'_>, Serverbound, Game;
        "custom_payload" => ServerboundCustomPayload<'_>, Serverbound, Game;
        "pong" => ServerboundPong, Serverbound, Game;
        "resource_pack" => ServerboundResourcePack, Serverbound, Game;
        "custom_click_action" => ServerboundCustomClickAction<'_>, Serverbound, Game;
        "block_entity_tag_query" => ServerboundBlockEntityTagQuery, Serverbound, Game;
        "bundle_item_selected" => ServerboundSelectBundleItem, Serverbound, Game;
        "change_difficulty" => ServerboundChangeDifficulty, Serverbound, Game;
        "change_game_mode" => ServerboundChangeGameMode, Serverbound, Game;
        "chat_ack" => ServerboundChatAck, Serverbound, Game;
        "chat_command" => ServerboundChatCommand<'_>, Serverbound, Game;
        "chat_command_signed" => ServerboundChatCommandSigned<'_>, Serverbound, Game;
        "chat" => ServerboundChat<'_>, Serverbound, Game;
        "chat_session_update" => ServerboundChatSessionUpdate, Serverbound, Game;
        "chunk_batch_received" => ServerboundChunkBatchReceived, Serverbound, Game;
        "client_information" => ServerboundClientInformation<'_>, Serverbound, Game;
        "configuration_acknowledged" => ServerboundConfigurationAcknowledged, Serverbound, Game;
        "container_button_click" => ServerboundContainerButtonClick, Serverbound, Game;
        "container_click" => ServerboundContainerClick, Serverbound, Game;
        "container_close" => ServerboundContainerClose, Serverbound, Game;
        "container_slot_state_changed" => ServerboundContainerSlotStateChanged, Serverbound, Game;
        "edit_book" => ServerboundEditBook<'_>, Serverbound, Game;
        "keep_alive" => ServerboundKeepAlive, Serverbound, Game;
        "move_player_pos" => ServerboundMovePlayerPos, Serverbound, Game;
        "move_player_pos_rot" => ServerboundMovePlayerPosRot, Serverbound, Game;
        "move_player_rot" => ServerboundMovePlayerRot, Serverbound, Game;
        "move_player_status_only" => ServerboundMovePlayerStatusOnly, Serverbound, Game;
        "pick_item_from_block" => ServerboundPickItemFromBlock, Serverbound, Game;
        "pick_item_from_entity" => ServerboundPickItemFromEntity, Serverbound, Game;
        "place_recipe" => ServerboundPlaceRecipe, Serverbound, Game;
        "player_action" => ServerboundPlayerAction, Serverbound, Game;
        "rename_item" => ServerboundRenameItem<'_>, Serverbound, Game;
        "select_trade" => ServerboundSelectTrade, Serverbound, Game;
        "recipe_book_change_settings" => ServerboundRecipeBookChangeSettings, Serverbound, Game;
        "recipe_book_seen_recipe" => ServerboundRecipeBookSeenRecipe, Serverbound, Game;
        "seen_advancements" => ServerboundSeenAdvancements, Serverbound, Game;
        "set_carried_item" => ServerboundSetCarriedItem, Serverbound, Game;
        "set_creative_mode_slot" => ServerboundSetCreativeModeSlot, Serverbound, Game;
        "use_item_on" => ServerboundUseItemOn, Serverbound, Game;
    }
}

#[test]
fn every_typed_row_is_pinned() {
    let pins: Pins = [
        handshake_and_status(),
        login(),
        configuration(),
        play_clientbound(),
        play_serverbound(),
    ]
    .concat();
    let pinned: BTreeSet<(&str, &str)> = pins
        .iter()
        .map(|&(state, side, name)| {
            let table = TABLES
                .iter()
                .find(|table| table.state == state && table.side == side)
                .unwrap_or_else(|| panic!("no table for {state:?} {side:?}"));
            (table.name, name)
        })
        .collect();
    assert_eq!(pinned.len(), pins.len(), "a structure is pinned twice");
    let typed: BTreeSet<(&str, &str)> = TABLES
        .iter()
        .flat_map(|table| table.typed.iter().map(|&name| (table.name, name)))
        .collect();
    assert_eq!(
        typed.difference(&pinned).collect::<Vec<_>>(),
        Vec::<&(&str, &str)>::new(),
        "typed rows without a pin"
    );
    assert_eq!(
        pinned.difference(&typed).collect::<Vec<_>>(),
        Vec::<&(&str, &str)>::new(),
        "pins without a typed row"
    );
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
    }
}
