//! Bedrock Edition wire encoding: varints, little-endian primitives, byte arrays and the packet header.
#![no_std]

extern crate alloc;

mod be_i32;
mod codec;
mod counted_list;
mod error;
#[cfg(feature = "glam")]
mod glam;
mod network_block_pos;
mod network_chunk_pos;
mod packet;
mod packet_header;
mod primitives;
mod slice;
mod string;
mod tag_conversions;
mod traits;
mod var_int;

pub use be_i32::BeI32;
pub use codec::{from_slice, to_bytes};
pub use counted_list::{decode_counted_list, encode_counted_list};
pub use error::Error;
pub use mcrs_minecraft_bedrock_wire_macros::{BedrockDecode, BedrockEncode};
pub use network_block_pos::NetworkBlockPos;
pub use network_chunk_pos::NetworkChunkPos;
pub use packet::{Packet, decode_packet, encode_packet};
pub use packet_header::PacketHeader;
pub use traits::{BedrockDecode, BedrockEncode};
pub use var_int::{VarI32, VarI64, VarU32, VarU64};

#[doc(hidden)]
pub mod __private {
    pub use alloc::vec::Vec;
}
