#![doc = include_str!("../README.md")]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_import_braces,
    unreachable_pub,
    clippy::dbg_macro
)]

extern crate self as mcrs_minecraft_wire;

/// Used only by macros. Not public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::{Context, Decode, Encode, Error, ErrorKind, Result, VarInt};
}

mod bounded;
mod error;
mod impls;
mod nested;
mod raw;
mod var_int;
mod var_long;

use std::io::Write;

pub use bounded::Bounded;
pub use error::{Context, Error, ErrorKind, Result};
pub use mcrs_minecraft_wire_macros::{Decode, Encode};
pub use nested::{MAX_NESTING, nested};
pub use raw::RawBytes;
pub use var_int::VarInt;
pub use var_long::VarLong;

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
/// A generic type bounds the type of each field by `Encode`, so a type
/// parameter that only selects the form of a field needs no bound itself.
///
/// ```
/// use mcrs_minecraft_wire::Encode;
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
/// [macro]: mcrs_minecraft_wire_macros::Encode
pub trait Encode {
    /// Writes this object to the provided writer.
    ///
    /// If this type also implements [`Decode`] then successful calls to this
    /// function returning `Ok(())` must always successfully [`decode`] using
    /// the data that was written to the writer. The exact number of bytes
    /// that were originally written must be consumed during the decoding.
    ///
    /// [`decode`]: Decode::decode
    fn encode(&self, w: impl Write) -> Result<()>;

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
    fn encode_slice(slice: &[Self], mut w: impl Write) -> Result<()>
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
/// use mcrs_minecraft_wire::{Decode, ErrorKind};
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
///
/// let error = MyEnum::decode(&mut &[2][..]).unwrap_err();
/// assert!(matches!(error.kind(), ErrorKind::UnknownTag { tag: 2, .. }));
/// ```
///
/// [macro]: mcrs_minecraft_wire_macros::Decode
pub trait Decode<'a>: Sized {
    /// Reads this object from the provided byte slice.
    ///
    /// Implementations of `Decode` are expected to shrink the slice from the
    /// front as bytes are read.
    fn decode(r: &mut &'a [u8]) -> Result<Self>;
}
