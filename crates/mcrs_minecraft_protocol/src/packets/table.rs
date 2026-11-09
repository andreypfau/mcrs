use crate::{ConnectionState, Decode, Packet, PacketSide};

pub const fn row_of(names: &[&str], name: &str) -> Option<i32> {
    let mut row = 0;
    let mut rest = names;
    while let [first, tail @ ..] = rest {
        if same_bytes(first.as_bytes(), name.as_bytes()) {
            return Some(row);
        }
        row += 1;
        rest = tail;
    }
    None
}

const fn same_bytes(mut a: &[u8], mut b: &[u8]) -> bool {
    loop {
        match (a, b) {
            ([], []) => return true,
            ([x, a_tail @ ..], [y, b_tail @ ..]) if *x == *y => (a, b) = (a_tail, b_tail),
            _ => return false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Table {
    pub name: &'static str,
    pub state: ConnectionState,
    pub side: PacketSide,
    pub names: &'static [&'static str],
    pub typed: &'static [&'static str],
}

#[derive(Debug)]
pub enum Decoded<P> {
    Packet(P),
    NotImplemented(&'static str),
    Unknown(i32),
}

/// A typed row of a table. The row is named by its packet type at the `'static` lifetime, so it
/// can be the parameter of an event, and decodes to the type at the lifetime of a frame.
pub trait Row: 'static {
    type Packet<'a>: Packet + Decode<'a>;
}

/// Receives the type of the row that a frame decodes as.
pub trait Visit {
    type Output;

    fn row<R: Row>(&mut self) -> Self::Output;
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
                "key" => ClientboundHello<'a>,
                "login_finished" => ClientboundLoginFinished<'a>,
                "setup_compression" => LoginCompression,
                "custom_query",
                "cookie_request",
                "apply_options",
            }
            login_serverbound: ServerboundLoginPacket<'a>, Login, Serverbound in ($crate::packets::login::serverbound) {
                "hello" => ServerboundHello<'a>,
                "key" => ServerboundKey<'a>,
                "custom_query_answer" => ServerboundCustomQueryAnswer<'a>,
                "login_acknowledged" => ServerboundLoginAcknowledged,
                "cookie_response" => ServerboundCookieResponse<'a>,
                "options_applied",
            }
            configuration_clientbound: ClientboundConfigurationPacket<'a>, Configuration, Clientbound in ($crate::packets::configuration::clientbound) {
                "cookie_request",
                "custom_payload" => ClientboundCustomPayload<'a>,
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
            game_clientbound: ClientboundGamePacket, Game, Clientbound in ($crate::packets::game::clientbound) {
                "bundle_delimiter" => ClientboundBundleDelimiter,
                "add_entity",
                "animate",
                "award_stats",
                "block_changed_ack",
                "block_destruction",
                "block_entity_data",
                "block_event",
                "block_update",
                "boss_event",
                "change_difficulty",
                "chunk_batch_finished",
                "chunk_batch_start",
                "chunks_biomes",
                "clear_titles",
                "command_suggestions",
                "commands",
                "container_close",
                "container_set_content",
                "container_set_data",
                "container_set_slot",
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
                "entity_event",
                "entity_position_sync",
                "explode",
                "add_transient_block",
                "forget_level_chunk",
                "game_event",
                "game_rule_values",
                "game_test_highlight_pos",
                "mount_screen_open",
                "hurt_animation",
                "initialize_border",
                "keep_alive" => ClientboundKeepAlive,
                "level_chunk_with_light",
                "level_event",
                "level_particles",
                "light_update",
                "login",
                "low_disk_space_warning",
                "map_item_data",
                "merchant_offers",
                "move_entity_pos",
                "move_entity_pos_rot",
                "move_minecart_along_track",
                "move_entity_rot",
                "move_vehicle",
                "open_book",
                "open_screen",
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
                "player_info_update",
                "player_look_at",
                "player_position",
                "player_rotation",
                "recipe_book_add",
                "recipe_book_remove",
                "recipe_book_settings",
                "remove_entities",
                "remove_mob_effect",
                "reset_score",
                "resource_pack_pop",
                "resource_pack_push",
                "post_effects",
                "respawn",
                "rotate_head",
                "section_blocks_update",
                "select_advancements_tab",
                "server_data",
                "set_action_bar_text",
                "set_border_center",
                "set_border_lerp_size",
                "set_border_size",
                "set_border_warning_delay",
                "set_border_warning_distance",
                "set_camera",
                "set_chunk_cache_center",
                "set_chunk_cache_radius",
                "set_cursor_item",
                "set_default_spawn_position",
                "set_display_objective",
                "set_entity_data",
                "set_entity_link",
                "set_entity_motion",
                "set_equipment",
                "set_experience",
                "set_health",
                "set_held_slot",
                "set_objective",
                "set_passengers",
                "set_player_inventory",
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
                "system_chat",
                "tab_list",
                "tag_query",
                "take_item_entity",
                "teleport_entity",
                "test_instance_block_status",
                "ticking_state",
                "ticking_step",
                "transfer",
                "update_advancements",
                "update_attributes",
                "update_mob_effect",
                "update_recipes",
                "update_tags",
                "projectile_power",
                "custom_report_details",
                "server_links",
                "waypoint",
                "clear_dialog",
                "show_dialog",
            }
            game_serverbound: ServerboundGamePacket, Game, Serverbound in ($crate::packets::game::serverbound) {
                "accept_teleportation",
                "attack",
                "block_entity_tag_query",
                "bundle_item_selected",
                "change_difficulty",
                "change_game_mode",
                "chat_ack",
                "chat_command",
                "chat_command_signed",
                "chat",
                "chat_session_update",
                "chunk_batch_received",
                "client_command",
                "client_tick_end",
                "client_information",
                "command_suggestion",
                "configuration_acknowledged" => ServerboundConfigurationAcknowledged,
                "container_button_click",
                "container_click",
                "container_close",
                "container_slot_state_changed",
                "cookie_response",
                "custom_payload",
                "debug_subscription_request",
                "edit_book",
                "entity_tag_query",
                "interact",
                "jigsaw_generate",
                "keep_alive" => ServerboundKeepAlive,
                "lock_difficulty",
                "move_player_pos",
                "move_player_pos_rot",
                "move_player_rot",
                "move_player_status_only",
                "move_vehicle",
                "paddle_boat",
                "pick_item_from_block",
                "pick_item_from_entity",
                "ping_request",
                "place_recipe",
                "player_abilities",
                "player_action",
                "player_command",
                "player_input",
                "player_loaded",
                "pong",
                "punch",
                "recipe_book_change_settings",
                "recipe_book_seen_recipe",
                "rename_item",
                "resource_pack",
                "seen_advancements",
                "select_trade",
                "set_beacon",
                "set_carried_item",
                "set_command_block",
                "set_command_minecart",
                "set_creative_mode_slot",
                "set_game_rule",
                "set_jigsaw_block",
                "set_structure_block",
                "set_test_block",
                "sign_update",
                "spectator_action",
                "teleport_to_entity",
                "test_instance_block_action",
                "use_item_on",
                "use_item",
                "custom_click_action",
            }
        }
    };
}

macro_rules! row {
    ($rows:ident, $ty:ident) => {
        $rows::$ty
    };
    ($rows:ident, $ty:ident, $lt:lifetime) => {
        $rows::$ty<'static>
    };
}

macro_rules! impl_row {
    ($rows:ident, $ty:ident) => {
        impl $crate::packets::table::Row for $rows::$ty {
            type Packet<'a> = $rows::$ty;
        }
    };
    ($rows:ident, $ty:ident, $lt:lifetime) => {
        impl $crate::packets::table::Row for $rows::$ty<'static> {
            type Packet<'a> = $rows::$ty<'a>;
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
                use $($module)* as rows;

                pub const NAMES: &[&str] = &[$($name),*];
                pub const TYPED: &[&str] = &[$($(typed_name!($name, $ty),)?)*];
                pub const STATE: $crate::ConnectionState = $crate::ConnectionState::$state;
                pub const SIDE: $crate::PacketSide = $crate::PacketSide::$side;
                pub const TABLE: $crate::packets::table::Table = $crate::packets::table::Table {
                    name: stringify!($table),
                    state: STATE,
                    side: SIDE,
                    names: NAMES,
                    typed: TYPED,
                };

                $($(
                    impl $(<$lt>)? $crate::Packet for rows::$ty $(<$lt>)? {
                        const ID: i32 = const {
                            let row = $crate::packets::table::row_of(NAMES, $name);
                            assert!(row.is_some(), "the name is not a row of its table");
                            match row {
                                Some(row) => row,
                                None => -1,
                            }
                        };
                        const NAME: &'static str = $name;
                        const SIDE: $crate::PacketSide = SIDE;
                        const STATE: $crate::ConnectionState = STATE;
                    }

                    impl_row!(rows, $ty $(, $lt)?);
                )?)*

                #[derive(Debug)]
                pub enum $enum $(<$tlt>)? {
                    $($($ty(rows::$ty $(<$lt>)?),)?)*
                }

                // A linear chain over this table's typed rows; the dispatcher
                // generated from the exported rows by another crate replaces it.
                pub fn decode$(<$tlt>)?(
                    id: i32,
                    r: &mut &$($tlt)? [u8],
                ) -> crate::Result<$crate::packets::table::Decoded<$enum $(<$tlt>)?>> {
                    $($(
                        if id == <rows::$ty as $crate::Packet>::ID {
                            return <rows::$ty as $crate::Decode>::decode(r)
                                .map($enum::$ty)
                                .map($crate::packets::table::Decoded::Packet)
                                .map_err(|error| {
                                    error.with_context(concat!("decoding ", $name, " (", stringify!($table), ")"))
                                });
                        }
                    )?)*
                    Ok(match usize::try_from(id).ok().and_then(|row| NAMES.get(row)) {
                        Some(name) => {
                            *r = &[];
                            $crate::packets::table::Decoded::NotImplemented(name)
                        }
                        None => $crate::packets::table::Decoded::Unknown(id),
                    })
                }

                /// Decodes the frame as [`decode`] does, and hands the type of its row to the
                /// visitor in place of the packet.
                pub fn visit<V: $crate::packets::table::Visit>(
                    id: i32,
                    r: &mut &[u8],
                    visitor: &mut V,
                ) -> crate::Result<$crate::packets::table::Decoded<V::Output>> {
                    use $crate::packets::table::Decoded;
                    Ok(match decode(id, r)? {
                        Decoded::Packet(packet) => Decoded::Packet(match packet {
                            $($($enum::$ty(_) => visitor.row::<row!(rows, $ty $(, $lt)?)>(),)?)*
                        }),
                        Decoded::NotImplemented(name) => Decoded::NotImplemented(name),
                        Decoded::Unknown(id) => Decoded::Unknown(id),
                    })
                }
            }
        )*

        pub const TABLES: &[Table] = &[$($table::TABLE,)*];
    };
}

for_each_packet_table!(tables);

#[doc(hidden)]
#[macro_export]
macro_rules! __table_row {
    ($rows:ident, $ty:ident) => {
        $rows::$ty
    };
    ($rows:ident, $ty:ident, $lt:lifetime) => {
        $rows::$ty<'static>
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __impl_table_row {
    ($rows:ident, $ty:ident) => {
        impl $crate::packets::table::Row for $rows::$ty {
            type Packet<'a> = $rows::$ty;
        }
    };
    ($rows:ident, $ty:ident, $lt:lifetime) => {
        impl $crate::packets::table::Row for $rows::$ty<'static> {
            type Packet<'a> = $rows::$ty<'a>;
        }
    };
}

/// Types more rows of a table of this crate in another crate. The table keeps its names, and
/// the rows this crate types decode as they do here.
#[macro_export]
macro_rules! extend_packet_table {
    ($($table:ident: $enum:ident<'a> extends $($base:ident)::+ in ($($module:tt)*) {
        $($name:literal => $ty:ident $(<$lt:lifetime>)?,)*
    })*) => {
        $(
            pub mod $table {
                use $($module)* as rows;
                use $($base)::+ as base;

                pub const NAMES: &[&str] = base::NAMES;
                const OWN: &[&str] = &[$($name),*];
                #[expect(
                    clippy::indexing_slicing,
                    reason = "evaluated at compile time, where an index past the end fails the build"
                )]
                const ALL_TYPED: [&str; base::TYPED.len() + OWN.len()] = {
                    let mut out = [""; base::TYPED.len() + OWN.len()];
                    let mut i = 0;
                    while i < base::TYPED.len() {
                        out[i] = base::TYPED[i];
                        i += 1;
                    }
                    let mut j = 0;
                    while j < OWN.len() {
                        out[i + j] = OWN[j];
                        j += 1;
                    }
                    out
                };
                pub const TYPED: &[&str] = &ALL_TYPED;
                pub const STATE: $crate::ConnectionState = base::STATE;
                pub const SIDE: $crate::PacketSide = base::SIDE;
                pub const TABLE: $crate::packets::table::Table = $crate::packets::table::Table {
                    name: base::TABLE.name,
                    state: STATE,
                    side: SIDE,
                    names: NAMES,
                    typed: TYPED,
                };

                $(
                    impl $(<$lt>)? $crate::Packet for rows::$ty $(<$lt>)? {
                        const ID: i32 = const {
                            let row = $crate::packets::table::row_of(NAMES, $name);
                            assert!(row.is_some(), "the name is not a row of its table");
                            assert!(
                                $crate::packets::table::row_of(base::TYPED, $name).is_none(),
                                "the row is typed by the base table"
                            );
                            match row {
                                Some(row) => row,
                                None => -1,
                            }
                        };
                        const NAME: &'static str = $name;
                        const SIDE: $crate::PacketSide = SIDE;
                        const STATE: $crate::ConnectionState = STATE;
                    }

                    $crate::__impl_table_row!(rows, $ty $(, $lt)?);
                )*

                #[derive(Debug)]
                pub enum $enum<'a> {
                    Base(base::$enum),
                    $($ty(rows::$ty $(<$lt>)?),)*
                }

                // A linear chain over this table's typed rows, as in the base table.
                pub fn decode<'a>(
                    id: i32,
                    r: &mut &'a [u8],
                ) -> $crate::Result<$crate::packets::table::Decoded<$enum<'a>>> {
                    use $crate::packets::table::Decoded;
                    $(
                        if id == <rows::$ty as $crate::Packet>::ID {
                            return <rows::$ty as $crate::Decode>::decode(r)
                                .map($enum::$ty)
                                .map(Decoded::Packet)
                                .map_err(|error| {
                                    error.with_context(concat!("decoding ", $name, " (", stringify!($table), ")"))
                                });
                        }
                    )*
                    Ok(match base::decode(id, r)? {
                        Decoded::Packet(packet) => Decoded::Packet($enum::Base(packet)),
                        Decoded::NotImplemented(name) => Decoded::NotImplemented(name),
                        Decoded::Unknown(id) => Decoded::Unknown(id),
                    })
                }

                /// Decodes the frame as [`decode`] does, and hands the type of its row to the
                /// visitor in place of the packet.
                pub fn visit<V: $crate::packets::table::Visit>(
                    id: i32,
                    r: &mut &[u8],
                    visitor: &mut V,
                ) -> $crate::Result<$crate::packets::table::Decoded<V::Output>> {
                    $(
                        if id == <rows::$ty as $crate::Packet>::ID {
                            return <rows::$ty as $crate::Decode>::decode(r)
                                .map(|_| $crate::packets::table::Decoded::Packet(
                                    visitor.row::<$crate::__table_row!(rows, $ty $(, $lt)?)>(),
                                ))
                                .map_err(|error| {
                                    error.with_context(concat!("decoding ", $name, " (", stringify!($table), ")"))
                                });
                        }
                    )*
                    base::visit(id, r, visitor)
                }
            }
        )*
    };
}
