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

extern crate self as mcrs_minecraft_protocol;
/// Used only by macros. Not public API.
#[doc(hidden)]
pub mod __private {
    pub use anyhow::{Context, Result, anyhow, bail, ensure};

    pub use crate::var_int::VarInt;
    pub use crate::{Decode, Encode, Packet};
}

pub mod advancement;
mod block;
pub mod block_pos;
mod byte_angle;
pub mod chunk;
pub mod column_pos;
pub mod decode;
mod difficulty;
mod direction;
pub mod encode;
pub mod entity;
pub mod game_event;
pub mod game_mode;
mod global_pos;
mod hand;
pub mod handshake;
mod impls;
pub mod item;
pub mod light_codec;
mod lp_vec3;
pub mod packed_section_pos;
pub mod packets;
pub mod particle;
mod pos;
pub mod profile;
mod raw;
pub mod recipe;
pub mod registry;
pub mod resource_pack;
pub mod section;
pub mod setting;
mod teleport_flags;
/// Text components with the item stack template as the hover item.
pub mod text {
    pub use mcrs_minecraft_text::*;

    pub use crate::item::Text;
}
pub mod var_int;
mod var_long;

use std::io::Write;

use anyhow::Context;
pub use byte_angle::ByteAngle;
pub use chunk::ChunkData;
pub use chunk::LightData;
pub use column_pos::ColumnPos;
pub use decode::PacketDecoder;
use derive_more::{From, Into};
pub use difficulty::Difficulty;
pub use direction::Direction;
pub use encode::{PacketEncoder, WritePacket};
pub use game_event::GameEventKind;
pub use game_mode::GameMode;
pub use global_pos::GlobalPos;
pub use hand::Hand;
pub use item::ProtoStack;
pub use lp_vec3::LpVec3;
pub use mcrs_minecraft_core::Bounded;
pub use mcrs_minecraft_protocol_macros::{Decode, Encode, Packet};
pub use pos::Look;
pub use pos::MoveFlags;
pub use pos::Position;
pub use raw::RawBytes;
use serde::{Deserialize, Serialize};
pub use teleport_flags::PositionFlag;
pub use text::Text;
pub use var_int::VarInt;
pub use var_long::VarLong;
pub use {anyhow, bytes, mcrs_minecraft_nbt as nbt, uuid};

/// The maximum number of bytes in a single Minecraft packet.
pub const MAX_PACKET_SIZE: i32 = 2097152;

/// The Minecraft protocol version this library currently targets.
pub const PROTOCOL_VERSION: i32 = 777;

/// The stringified name of the Minecraft version this library currently
/// targets.
pub const MINECRAFT_VERSION: &str = "26.3";

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

/// The `Encode` trait allows objects to be written to the Minecraft protocol.
/// It is the inverse of [`Decode`].
///
/// # Deriving
///
/// This trait can be implemented automatically for structs and enums by using
/// the [`Encode`][macro] derive macro. All components of the type must
/// implement `Encode`. Components are encoded in the order they appear in the
/// type definition.
///
/// For enums, the variant to encode is marked by a leading [`VarInt`]
/// discriminant (tag). The discriminant value can be changed using the
/// `#[packet(tag = ...)]` attribute on the variant in question. Discriminant
/// values are assigned to variants using rules similar to regular enum
/// discriminants.
///
/// ```
/// use mcrs_minecraft_protocol::Encode;
///
/// #[derive(Encode)]
/// struct MyStruct<'a> {
///     first: i32,
///     second: &'a str,
///     third: [f64; 3],
/// }
///
/// #[derive(Encode)]
/// enum MyEnum {
///     First,  // tag = 0
///     Second, // tag = 1
///     #[packet(tag = 25)]
///     Third, // tag = 25
///     Fourth, // tag = 26
/// }
///
/// let value = MyStruct {
///     first: 10,
///     second: "hello",
///     third: [1.5, 3.14, 2.718],
/// };
///
/// let mut buf = vec![];
/// value.encode(&mut buf).unwrap();
///
/// println!("{buf:?}");
/// ```
///
/// [macro]: mcrs_minecraft_protocol_macros::Encode
/// [`VarInt`]: var_int::VarInt
pub trait Encode {
    /// Writes this object to the provided writer.
    ///
    /// If this type also implements [`Decode`] then successful calls to this
    /// function returning `Ok(())` must always successfully [`decode`] using
    /// the data that was written to the writer. The exact number of bytes
    /// that were originally written must be consumed during the decoding.
    ///
    /// [`decode`]: Decode::decode
    fn encode(&self, w: impl Write) -> anyhow::Result<()>;

    /// Like [`Encode::encode`], except that a whole slice of values is encoded.
    ///
    /// This method must be semantically equivalent to encoding every element of
    /// the slice in sequence with no leading length prefix (which is exactly
    /// what the default implementation does), but a more efficient
    /// implementation may be used.
    ///
    /// This method is important for some types like `u8` where the entire slice
    /// can be encoded in a single call to [`write_all`]. Because impl
    /// specialization is unavailable in stable Rust at the time of writing,
    /// we must make the slice specialization part of this trait.
    ///
    /// [`write_all`]: Write::write_all
    fn encode_slice(slice: &[Self], mut w: impl Write) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        for value in slice {
            value.encode(&mut w)?;
        }

        Ok(())
    }
}

/// The `Decode` trait allows objects to be read from the Minecraft protocol. It
/// is the inverse of [`Encode`].
///
/// `Decode` is parameterized by a lifetime. This allows the decoded value to
/// borrow data from the byte slice it was read from.
///
/// # Deriving
///
/// This trait can be implemented automatically for structs and enums by using
/// the [`Decode`][macro] derive macro. All components of the type must
/// implement `Decode`. Components are decoded in the order they appear in the
/// type definition.
///
/// For enums, the variant to decode is determined by a leading [`VarInt`]
/// discriminant (tag). The discriminant value can be changed using the
/// `#[packet(tag = ...)]` attribute on the variant in question. Discriminant
/// values are assigned to variants using rules similar to regular enum
/// discriminants.
///
/// ```
/// use mcrs_minecraft_protocol::Decode;
///
/// #[derive(PartialEq, Debug, Decode)]
/// struct MyStruct {
///     first: i32,
///     second: MyEnum,
/// }
///
/// #[derive(PartialEq, Debug, Decode)]
/// enum MyEnum {
///     First,  // tag = 0
///     Second, // tag = 1
///     #[packet(tag = 25)]
///     Third, // tag = 25
///     Fourth, // tag = 26
/// }
///
/// let mut r: &[u8] = &[0, 0, 0, 0, 26];
///
/// let value = MyStruct::decode(&mut r).unwrap();
/// let expected = MyStruct {
///     first: 0,
///     second: MyEnum::Fourth,
/// };
///
/// assert_eq!(value, expected);
/// assert!(r.is_empty());
/// ```
///
/// [macro]: mcrs_minecraft_protocol_macros::Decode
/// [`VarInt`]: var_int::VarInt
pub trait Decode<'a>: Sized {
    /// Reads this object from the provided byte slice.
    ///
    /// Implementations of `Decode` are expected to shrink the slice from the
    /// front as bytes are read.
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self>;
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
    fn encode_with_id(&self, mut w: impl Write) -> anyhow::Result<()>
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

/// The statein  which a packet is used.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ConnectionState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Game,
}
