/// A newtype over an `i32` or a `codec::Bounded` int, one VarInt on the wire.
macro_rules! var_int_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl mcrs_minecraft_wire::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> mcrs_minecraft_wire::Result<()> {
                mcrs_minecraft_wire::VarInt(self.0.into()).encode(w)
            }
        }

        impl mcrs_minecraft_wire::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> mcrs_minecraft_wire::Result<Self> {
                Ok($ty(mcrs_minecraft_wire::VarInt::decode(r)?.0.into()))
            }
        }
    )*};
}
pub(crate) use var_int_wire;

/// A field-less enum, its ordinal as one VarInt; out-of-range ids read as the
/// first variant.
macro_rules! ordinal_enum_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl mcrs_minecraft_wire::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> mcrs_minecraft_wire::Result<()> {
                mcrs_minecraft_wire::VarInt(*self as i32).encode(w)
            }
        }

        impl mcrs_minecraft_wire::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> mcrs_minecraft_wire::Result<Self> {
                let id = mcrs_minecraft_wire::VarInt::decode(r)?.0;
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
