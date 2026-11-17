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

pub mod decode;
pub mod encode;
pub mod frame;
pub mod handshake;
pub mod packets;
pub mod profile;
mod raw;
mod record;
pub mod registry;
pub mod resource_pack;
pub mod setting;

use std::io::Write;

pub use decode::PacketDecoder;
use derive_more::{From, Into};
pub use encode::{PacketEncoder, WritePacket};
pub use mcrs_minecraft_wire::{
    Bounded, Context, Decode, Encode, Error, ErrorKind, RawBytes, Result, VarInt, VarLong,
};
pub use raw::EncodedBody;
pub use record::{RECORD_LENGTH_BYTES, write_record};
use serde::{Deserialize, Serialize};
pub use {anyhow, bytes, mcrs_minecraft_nbt as nbt, mcrs_minecraft_wire as wire, uuid};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, From, Into)]
pub struct CompressionThreshold(pub i32);

impl Default for CompressionThreshold {
    fn default() -> Self {
        Self(-1)
    }
}

pub trait Packet: std::fmt::Debug {
    const ID: i32;
    const NAME: &'static str;
    const SIDE: PacketSide;
    const STATE: ConnectionState;

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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PacketSide {
    Clientbound,
    Serverbound,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub enum ConnectionState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Game,
}
