mod common;

mod chunk_wire;
mod column_pos_wire;
mod decoder_advances_reader;
mod entity_movement_codecs;
mod entity_packets;
mod frames_golden;
#[allow(dead_code)]
#[path = "../../../mcrs_minecraft_core/tests/it/identifier_rows.rs"]
mod identifier_rows;
mod identifier_wire;
mod inventory_packets;
mod item;
mod join_packets;
mod light_update_fixtures;
mod lp_vec3;
mod packet_tables;
mod serverbound_game_packets;
mod tag_payload;
mod text;
