//! Wire forms of the data component types. The types themselves carry only
//! serde; every `Encode`/`Decode` impl of their `Wire` form lives here.

pub use kind::decode_component_value;
pub use patch::{decode_delimited_patch, encode_delimited_patch};
pub use stack::{DelimitedStack, HashedStack};

mod attribute;
mod banner;
mod book;
mod combat;
mod common;
mod consume;
mod entity_data;
mod enums;
mod equippable;
mod fireworks;
mod fuel;
mod instrument;
mod kind;
mod lodestone;
mod nbt_wire;
mod nested;
mod painting;
mod patch;
mod potion;
mod predicate;
mod profile;
mod registry_ref;
mod scalar;
mod simple;
mod sound;
mod stack;
mod text;
mod text_component;
mod trim;
mod unit;
mod variants;

/// A newtype over an `Encode + Decode` inner value.
macro_rules! newtype_wire {
    ($($ty:ty),* $(,)?) => {$(
        impl $crate::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                $crate::Encode::encode(&self.0, w)
            }
        }

        impl<'a> $crate::Decode<'a> for $ty {
            fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
                $crate::Decode::decode(r).map(Self)
            }
        }
    )*};
}
pub(crate) use newtype_wire;

/// A newtype over a plain `i32`, one VarInt on the wire.
macro_rules! var_int_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl $crate::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                $crate::VarInt(self.0).encode(w)
            }
        }

        impl $crate::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok($ty($crate::VarInt::decode(r)?.0))
            }
        }
    )*};
}
pub(crate) use var_int_wire;

/// A newtype over a `codec::Bounded` int, one VarInt on the wire.
macro_rules! bounded_var_int_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl $crate::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                $crate::VarInt(self.0.0).encode(w)
            }
        }

        impl $crate::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok($ty(mcrs_minecraft_core::codec::Bounded($crate::VarInt::decode(r)?.0)))
            }
        }
    )*};
}
pub(crate) use bounded_var_int_wire;

/// A field-less enum, its ordinal as one VarInt; out-of-range ids read as the
/// first variant.
macro_rules! ordinal_enum_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl $crate::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                $crate::VarInt(*self as i32).encode(w)
            }
        }

        impl $crate::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let id = $crate::VarInt::decode(r)?.0;
                Ok(usize::try_from(id)
                    .ok()
                    .and_then(|id| Self::ALL.get(id))
                    .copied()
                    .unwrap_or(Self::ALL[0]))
            }
        }
    )*};
}
pub(crate) use ordinal_enum_wire;

/// A value with no fields: nothing on the wire.
macro_rules! unit_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl $crate::Encode for $ty {
            fn encode(&self, _: impl std::io::Write) -> anyhow::Result<()> {
                Ok(())
            }
        }

        impl $crate::Decode<'_> for $ty {
            fn decode(_: &mut &[u8]) -> anyhow::Result<Self> {
                Ok($ty)
            }
        }
    )*};
}
pub(crate) use unit_wire;

/// A record whose fields follow each other on the wire in the listed order,
/// each in its own plain wire form.
macro_rules! record_wire {
    ($($ty:ty { $($field:ident),+ $(,)? }),* $(,)?) => {$(
        impl $crate::Encode for $ty {
            fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
                $($crate::Encode::encode(&self.$field, &mut w)?;)+
                Ok(())
            }
        }

        impl<'a> $crate::Decode<'a> for $ty {
            fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
                Ok(Self {
                    $($field: $crate::Decode::decode(r)?,)+
                })
            }
        }
    )*};
}
pub(crate) use record_wire;
