#![cfg_attr(
    test,
    expect(clippy::disallowed_macros, reason = "test input is written as JSON")
)]

mod binary_read_guard;
pub mod compound;
pub mod deserializer;
mod error;
mod nbt;
mod nbt_array;
pub mod nbt_compress;
mod nbt_flag;
pub mod serializer;
pub mod tag;
pub mod tag_deserializer;
mod tag_id;
pub mod tag_serializer;
#[cfg(test)]
mod test;
mod wire;

pub(crate) use binary_read_guard::{BinaryReadGuard, reading_binary};
pub use deserializer::{from_bytes, from_bytes_unnamed};
pub use error::{Error, MAX_DEPTH};
pub use nbt::{Nbt, get_nbt_string};
pub use nbt_array::{
    ArrayKind, ArrayVisitor, NBT_ARRAY_TAG, nbt_array, nbt_byte_array, nbt_int_array,
    nbt_long_array,
};
pub(crate) use nbt_array::{NBT_BYTE_ARRAY_TAG, NBT_INT_ARRAY_TAG, NBT_LONG_ARRAY_TAG};
pub use nbt_flag::nbt_flag;
pub use serializer::{to_bytes, to_bytes_named, to_bytes_unnamed};
pub use tag_deserializer::from_tag;
pub use tag_id::{
    BYTE_ARRAY_ID, BYTE_ID, COMPOUND_ID, DOUBLE_ID, END_ID, FLOAT_ID, INT_ARRAY_ID, INT_ID,
    LIST_ID, LONG_ARRAY_ID, LONG_ID, SHORT_ID, STRING_ID,
};
pub use tag_serializer::{to_nbt_compound, to_nbt_tag};
pub use wire::{WireDeserializer, decode_wire, encode_wire, read_wire};
