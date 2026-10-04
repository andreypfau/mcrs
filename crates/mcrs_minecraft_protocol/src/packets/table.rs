use crate::{ConnectionState, PacketSide};

pub const fn row_of(names: &[&str], name: &str) -> i32 {
    let mut row = 0;
    while row < names.len() {
        let (a, b) = (names[row].as_bytes(), name.as_bytes());
        if a.len() == b.len() {
            let mut i = 0;
            while i < a.len() && a[i] == b[i] {
                i += 1;
            }
            if i == a.len() {
                return row as i32;
            }
        }
        row += 1;
    }
    panic!("the name is not a row of its table")
}

#[derive(Debug, Clone, Copy)]
pub struct Table {
    pub name: &'static str,
    pub state: ConnectionState,
    pub side: PacketSide,
    pub names: &'static [&'static str],
    pub typed: &'static [&'static str],
}

#[macro_export]
macro_rules! for_each_packet_table {
    ($callback:ident) => {
        $callback! {
            handshake_serverbound: ServerboundHandshakePacket<'a>, Handshaking, Serverbound in ($crate::packets::intent::serverbound) {
                "intention" => ServerboundHandshake<'a>,
            }
            status_clientbound: ClientboundStatusPacket<'a>, Status, Clientbound in ($crate::packets::status::clientbound) {
                "status_response" => StatusResponse<'a>,
                "pong_response" => PongResponse,
            }
            status_serverbound: ServerboundStatusPacket, Status, Serverbound in ($crate::packets::status::serverbound) {
                "status_request" => StatusRequest,
                "ping_request" => PingRequest,
            }
            login_clientbound: ClientboundLoginPacket<'a>, Login, Clientbound in ($crate::packets::login::clientbound) {
                "login_disconnect" => ClientboundLoginDisconnect<'a>,
                "hello" => ClientboundHello<'a>,
                "login_finished" => ClientboundLoginFinished<'a>,
                "login_compression" => LoginCompression,
                "custom_query",
                "cookie_request",
            }
            login_serverbound: ServerboundLoginPacket<'a>, Login, Serverbound in ($crate::packets::login::serverbound) {
                "hello" => ServerboundHello<'a>,
                "key" => ServerboundKey<'a>,
                "custom_query_answer" => ServerboundCustomQueryAnswer<'a>,
                "login_acknowledged" => ServerboundLoginAcknowledged,
                "cookie_response" => ServerboundCookieResponse<'a>,
            }
            configuration_clientbound: ClientboundConfigurationPacket<'a>, Configuration, Clientbound in ($crate::packets::configuration::clientbound) {
                "cookie_request",
                "custom_payload",
                "disconnect" => ClientboundDisconnect,
                "finish_configuration" => ClientboundFinishConfiguration,
                "keep_alive" => ClientboundKeepAlive,
                "ping",
                "reset_chat",
                "registry_data" => ClientboundRegistryData<'a>,
                "resource_pack_pop",
                "resource_pack_push",
                "post_effects",
                "store_cookie",
                "transfer",
                "update_enabled_features",
                "update_tags" => ClientboundUpdateTags<'a>,
                "select_known_packs" => ClientboundSelectKnownPacks<'a>,
                "custom_report_details",
                "server_links",
                "clear_dialog",
                "show_dialog" => ClientboundShowDialog,
                "code_of_conduct",
            }
            configuration_serverbound: ServerboundConfigurationPacket<'a>, Configuration, Serverbound in ($crate::packets::configuration::serverbound) {
                "client_information" => ServerboundClientInformation<'a>,
                "cookie_response" => ServerboundCookieResponse<'a>,
                "custom_payload" => ServerboundCustomPayload<'a>,
                "finish_configuration" => ServerboundFinishConfiguration,
                "keep_alive" => ServerboundKeepAlive,
                "pong" => ServerboundPong,
                "resource_pack" => ServerboundResourcePack,
                "select_known_packs" => ServerboundSelectKnownPacks<'a>,
                "custom_click_action" => ServerboundCustomClickAction<'a>,
                "accept_code_of_conduct" => ServerboundAcceptCodeOfConduct,
            }
            game_clientbound: ClientboundGamePacket<'a>, Game, Clientbound in ($crate::packets::game::clientbound) {
                "bundle_delimiter",
                "add_entity" => ClientboundAddEntity,
                "animate",
                "award_stats",
                "block_changed_ack",
                "block_destruction" => ClientboundBlockDestruction,
                "block_entity_data",
                "block_event",
                "block_update" => ClientboundBlockUpdate,
                "boss_event",
                "change_difficulty",
                "chunk_batch_finished" => ClientboundChunkBatchFinished,
                "chunk_batch_start" => ClientboundChunkBatchStart,
                "chunks_biomes",
                "clear_titles",
                "command_suggestions",
                "commands",
                "container_close" => ClientboundContainerClose,
                "container_set_content" => ClientboundContainerSetContent,
                "container_set_data" => ClientboundContainerSetData,
                "container_set_slot" => ClientboundContainerSetSlot,
                "cookie_request",
                "cooldown",
                "custom_chat_completions",
                "custom_payload",
                "damage_event",
                "debug/block_value",
                "debug/chunk_value",
                "debug/entity_value",
                "debug/event",
                "debug_sample",
                "delete_chat",
                "disconnect" => ClientboundDisconnect,
                "disguised_chat",
                "entity_event" => ClientboundEntityEvent,
                "entity_position_sync" => ClientboundEntityPositionSync,
                "explode",
                "add_transient_block",
                "forget_level_chunk" => ClientboundForgetLevelChunk,
                "game_event" => ClientboundGameEvent,
                "game_rule_values",
                "game_test_highlight_pos",
                "mount_screen_open",
                "hurt_animation",
                "initialize_border",
                "keep_alive" => ClientboundKeepAlive,
                "level_chunk_with_light" => ClientboundLevelChunkWithLight<'a>,
                "level_event",
                "level_particles" => ClientboundLevelParticles,
                "light_update" => ClientboundLightUpdate<'a>,
                "login" => ClientboundLogin<'a>,
                "low_disk_space_warning",
                "map_item_data",
                "merchant_offers" => ClientboundMerchantOffers,
                "move_entity_pos" => ClientboundMoveEntityPos,
                "move_entity_pos_rot" => ClientboundMoveEntityPosRot,
                "move_minecart_along_track" => ClientboundMoveMinecartAlongTrack,
                "move_entity_rot" => ClientboundMoveEntityRot,
                "move_vehicle",
                "open_book",
                "open_screen" => ClientboundOpenScreen,
                "open_sign_editor",
                "ping",
                "pong_response",
                "place_ghost_recipe",
                "player_abilities",
                "player_chat",
                "player_combat_end",
                "player_combat_enter",
                "player_combat_kill",
                "player_info_remove",
                "player_info_update" => ClientboundPlayerInfoUpdate<'a>,
                "player_look_at",
                "player_position" => ClientboundPlayerPosition,
                "player_rotation",
                "recipe_book_add" => ClientboundRecipeBookAdd,
                "recipe_book_remove" => ClientboundRecipeBookRemove,
                "recipe_book_settings" => ClientboundRecipeBookSettings,
                "remove_entities" => ClientboundRemoveEntities,
                "remove_mob_effect",
                "reset_score",
                "resource_pack_pop",
                "resource_pack_push",
                "post_effects",
                "respawn" => ClientboundRespawn<'a>,
                "rotate_head" => ClientboundRotateHead,
                "section_blocks_update" => ClientboundSectionBlocksUpdate<'a>,
                "select_advancements_tab",
                "server_data",
                "set_action_bar_text",
                "set_border_center",
                "set_border_lerp_size",
                "set_border_size",
                "set_border_warning_delay",
                "set_border_warning_distance",
                "set_camera",
                "set_chunk_cache_center" => ClientboundSetChunkCacheCenter,
                "set_chunk_cache_radius" => ClientboundChunkCacheRadius,
                "set_cursor_item" => ClientboundSetCursorItem,
                "set_default_spawn_position",
                "set_display_objective",
                "set_entity_data" => ClientboundSetEntityData<'a>,
                "set_entity_link",
                "set_entity_motion",
                "set_equipment" => ClientboundSetEquipment,
                "set_experience",
                "set_health",
                "set_held_slot" => ClientboundSetHeldSlot,
                "set_objective",
                "set_passengers" => ClientboundSetPassengers,
                "set_player_inventory" => ClientboundSetPlayerInventory,
                "set_player_team",
                "set_score",
                "set_simulation_distance",
                "set_subtitle_text",
                "set_time",
                "set_title_text",
                "set_titles_animation",
                "sound_entity",
                "sound",
                "start_configuration" => ClientboundStartConfiguration,
                "stop_sound",
                "store_cookie",
                "swing_animation",
                "system_chat" => ClientboundSystemChatPacket,
                "tab_list",
                "tag_query",
                "take_item_entity" => ClientboundTakeItemEntity,
                "teleport_entity",
                "test_instance_block_status",
                "ticking_state",
                "ticking_step",
                "transfer",
                "update_advancements" => ClientboundUpdateAdvancements,
                "update_attributes" => ClientboundUpdateAttributes<'a>,
                "update_mob_effect",
                "update_recipes" => ClientboundUpdateRecipes,
                "update_tags",
                "projectile_power",
                "custom_report_details",
                "server_links",
                "waypoint",
                "clear_dialog",
                "show_dialog",
            }
            game_serverbound: ServerboundGamePacket<'a>, Game, Serverbound in ($crate::packets::game::serverbound) {
                "accept_teleportation" => ServerboundAcceptTeleportation,
                "attack" => ServerboundAttack,
                "block_entity_tag_query" => ServerboundBlockEntityTagQuery,
                "bundle_item_selected" => ServerboundSelectBundleItem,
                "change_difficulty" => ServerboundChangeDifficulty,
                "change_game_mode" => ServerboundChangeGameMode,
                "chat_ack" => ServerboundChatAck,
                "chat_command" => ServerboundChatCommand<'a>,
                "chat_command_signed" => ServerboundChatCommandSigned<'a>,
                "chat" => ServerboundChat<'a>,
                "chat_session_update" => ServerboundChatSessionUpdate,
                "chunk_batch_received" => ServerboundChunkBatchReceived,
                "client_command" => ServerboundClientCommand,
                "client_tick_end" => ServerboundClientTickEnd,
                "client_information" => ServerboundClientInformation<'a>,
                "command_suggestion",
                "configuration_acknowledged" => ServerboundConfigurationAcknowledged,
                "container_button_click" => ServerboundContainerButtonClick,
                "container_click" => ServerboundContainerClick,
                "container_close" => ServerboundContainerClose,
                "container_slot_state_changed" => ServerboundContainerSlotStateChanged,
                "cookie_response" => ServerboundCookieResponse<'a>,
                "custom_payload" => ServerboundCustomPayload<'a>,
                "debug_subscription_request",
                "edit_book" => ServerboundEditBook<'a>,
                "entity_tag_query",
                "interact" => ServerboundInteract,
                "jigsaw_generate",
                "keep_alive" => ServerboundKeepAlive,
                "lock_difficulty",
                "move_player_pos" => ServerboundMovePlayerPos,
                "move_player_pos_rot" => ServerboundMovePlayerPosRot,
                "move_player_rot" => ServerboundMovePlayerRot,
                "move_player_status_only" => ServerboundMovePlayerStatusOnly,
                "move_vehicle" => ServerboundMoveVehicle,
                "paddle_boat",
                "pick_item_from_block" => ServerboundPickItemFromBlock,
                "pick_item_from_entity" => ServerboundPickItemFromEntity,
                "ping_request",
                "place_recipe" => ServerboundPlaceRecipe,
                "player_abilities" => ServerboundPlayerAbilities,
                "player_action" => ServerboundPlayerAction,
                "player_command" => ServerboundPlayerCommand,
                "player_input" => ServerboundPlayerInput,
                "player_loaded" => ServerboundPlayerLoaded,
                "pong" => ServerboundPong,
                "punch" => ServerboundPunch,
                "recipe_book_change_settings" => ServerboundRecipeBookChangeSettings,
                "recipe_book_seen_recipe" => ServerboundRecipeBookSeenRecipe,
                "rename_item" => ServerboundRenameItem<'a>,
                "resource_pack" => ServerboundResourcePack,
                "seen_advancements" => ServerboundSeenAdvancements,
                "select_trade" => ServerboundSelectTrade,
                "set_beacon",
                "set_carried_item" => ServerboundSetCarriedItem,
                "set_command_block",
                "set_command_minecart",
                "set_creative_mode_slot" => ServerboundSetCreativeModeSlot,
                "set_game_rule",
                "set_jigsaw_block",
                "set_structure_block",
                "set_test_block",
                "sign_update",
                "spectator_action",
                "teleport_to_entity",
                "test_instance_block_action",
                "use_item_on" => ServerboundUseItemOn,
                "use_item" => ServerboundUseItem,
                "custom_click_action" => ServerboundCustomClickAction<'a>,
            }
        }
    };
}

macro_rules! typed_name {
    ($name:literal, $ty:ident) => {
        $name
    };
}

macro_rules! tables {
    ($($table:ident: $enum:ident $(<$tlt:lifetime>)?, $state:ident, $side:ident in ($($module:tt)*) {
        $($name:literal $(=> $ty:ident $(<$lt:lifetime>)?)?,)*
    })*) => {
        $(
            pub mod $table {
                #[allow(unused_imports, unused_import_braces)]
                use $($module)*::{$($($ty,)?)*};

                pub const NAMES: &[&str] = &[$($name),*];
                pub const TYPED: &[&str] = &[$($(typed_name!($name, $ty),)?)*];
                pub const STATE: $crate::ConnectionState = $crate::ConnectionState::$state;
                pub const SIDE: $crate::PacketSide = $crate::PacketSide::$side;

                $($(
                    impl $(<$lt>)? $crate::Packet for $ty $(<$lt>)? {
                        const ID: i32 = $crate::packets::table::row_of(NAMES, $name);
                        const NAME: &'static str = $name;
                        const SIDE: $crate::PacketSide = SIDE;
                        const STATE: $crate::ConnectionState = STATE;
                    }
                )?)*
            }
        )*

        pub const TABLES: &[Table] = &[$(
            Table {
                name: stringify!($table),
                state: $table::STATE,
                side: $table::SIDE,
                names: $table::NAMES,
                typed: $table::TYPED,
            },
        )*];
    };
}

for_each_packet_table!(tables);
