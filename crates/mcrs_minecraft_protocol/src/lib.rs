#![doc = include_str!("../README.md")]
#![deny(
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    rustdoc::missing_crate_level_docs,
    rustdoc::invalid_codeblock_attributes,
    rustdoc::invalid_rust_codeblocks,
    rustdoc::bare_urls,
    rustdoc::invalid_html_tags
)]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_import_braces,
    unreachable_pub,
    clippy::dbg_macro
)]

#[rustfmt::skip]
pub mod advancement;
pub mod block;
mod byte_angle;
pub mod chat;
pub mod chunk;
mod command_suggestion;
pub mod decode;
mod difficulty;
pub mod encode;
pub mod entity;
pub mod frame;
pub mod game_event;
mod hand;
pub mod handshake;
pub mod item;
pub mod light_codec;
mod lp_vec3;
pub mod packets;
mod plain_text;
mod pos;
pub mod profile;
mod raw;
pub mod recipe;
pub mod registry;
pub mod resource_pack;
pub mod section;
pub mod setting;
mod teleport_flags;
/// Text components with the item stack template as the hover item, in the named form a packet
/// carries.
pub mod text {
    pub use mcrs_minecraft_text::*;

    pub type Text = mcrs_minecraft_item::Text<mcrs_minecraft_registries::Named>;

    pub use crate::plain_text::{PlainTextError, named};
}

use std::io::Write;

pub use byte_angle::ByteAngle;
pub use chunk::ChunkData;
pub use chunk::LightData;
pub use command_suggestion::CommandSuggestion;
pub use decode::PacketDecoder;
use derive_more::{From, Into};
pub use difficulty::Difficulty;
pub use encode::{PacketEncoder, WritePacket};
pub use game_event::GameEventKind;
pub use hand::Hand;
pub use item::ProtoStack;
pub use lp_vec3::LpVec3;
pub use mcrs_minecraft_core::Direction;
pub use mcrs_minecraft_item::component::GlobalPos;
pub use mcrs_minecraft_wire::{
    Bounded, Context, Decode, Encode, Error, ErrorKind, RawBytes, Result, VarInt, VarLong,
};
pub use mcrs_voxel::ColumnPos;
pub use pos::MoveFlags;
pub use pos::Position;
pub use raw::EncodedBody;
use serde::{Deserialize, Serialize};
pub use teleport_flags::{PositionFlag, PositionFlags};
pub use text::Text;
pub use {anyhow, bytes, mcrs_minecraft_nbt as nbt, mcrs_minecraft_wire as wire, uuid};

/// How large a packet should be before it is compressed by the packet encoder.
///
/// If the inner value is >= 0, then packets with encoded lengths >= to this
/// value will be compressed. If the value is negative, then compression is
/// disabled and no packets are compressed.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, From, Into)]
pub struct CompressionThreshold(pub i32);

/// No compression.
impl Default for CompressionThreshold {
    fn default() -> Self {
        Self(-1)
    }
}

/// Types considered to be Minecraft packets.
///
/// In serialized form, a packet begins with a [`VarInt`] packet ID followed by
/// the body of the packet. If present, the implementations of [`Encode`] and
/// [`Decode`] on `Self` are expected to only encode/decode the _body_ of this
/// packet without the leading ID.
pub trait Packet: std::fmt::Debug {
    /// The leading VarInt ID of this packet.
    const ID: i32;
    /// The name of this packet for debugging purposes.
    const NAME: &'static str;
    /// The side this packet is intended for.
    const SIDE: PacketSide;
    /// The state in which this packet is used.
    const STATE: ConnectionState;

    /// Encodes this packet's VarInt ID first, followed by the packet's body.
    fn encode_with_id(&self, mut w: impl Write) -> Result<()>
    where
        Self: Encode,
    {
        VarInt(Self::ID)
            .encode(&mut w)
            .context("failed to encode packet ID")?;

        self.encode(w)
    }
}

/// The side a packet is intended for.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PacketSide {
    /// Server -> Client
    Clientbound,
    /// Client -> Server
    Serverbound,
}

/// The state in which a packet is used.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub enum ConnectionState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Game,
}
