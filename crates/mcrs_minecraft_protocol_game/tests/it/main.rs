#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
#![expect(clippy::disallowed_types, reason = "test input is written as JSON")]

mod common;

mod alloc_free;
mod block_entity_wire;
mod block_event_wire;

mod chat_wire;
mod chunk_wire;
mod column_pos_wire;
mod decoder_advances_reader;
mod entity_movement_codecs;
mod entity_packets;
mod inventory_packets;
mod item;
mod join_packets;
mod light_update_fixtures;
mod lp_vec3;
mod packet_tables;
mod registry_ids;
mod serverbound_game_packets;
mod text;
