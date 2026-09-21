//! Wire forms of the data component types. The types themselves carry only
//! serde; every `Encode`/`Decode`/`EncodeCtx`/`DecodeCtx` impl lives here.

mod scalar;

/// A newtype over an `Encode + Decode` inner value.
macro_rules! newtype_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl $crate::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                $crate::Encode::encode(&self.0, w)
            }
        }

        impl<'a> $crate::Decode<'a> for $ty {
            fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
                $crate::Decode::decode(r).map($ty)
            }
        }

        $crate::item::ctx::ctx_free!($ty);
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

        $crate::item::ctx::ctx_free!($ty);
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

        $crate::item::ctx::ctx_free!($ty);
    )*};
}
pub(crate) use bounded_var_int_wire;
