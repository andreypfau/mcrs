use std::io::Write;

use mcrs_minecraft_core::codec::Bounded;

use crate::item::component::enums::*;
use crate::item::wire::{newtype_wire, ordinal_enum_wire};
use crate::{Decode, Encode, VarInt};

ordinal_enum_wire!(
    Rarity,
    DyeColor,
    MapPostProcessing,
    SwingAnimationKind,
    FoxVariant,
    AxolotlVariant
);
newtype_wire!(
    AttackAnimation,
    InteractAnimation,
    Dye,
    BaseColor,
    WolfCollar,
    TropicalFishBaseColor,
    TropicalFishPatternColor,
    CatCollar,
    SheepColor,
    ShulkerColor,
    CushionColor,
);

/// The wire id is the ordinal; an out-of-range id is clamped or wrapped.
macro_rules! continuous_enum_wire {
    ($($name:ident [$strategy:ident]),* $(,)?) => {$(
        impl Encode for $name {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                VarInt(*self as i32).encode(w)
            }
        }

        impl Decode<'_> for $name {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let id = VarInt::decode(r)?.0;
                let len = Self::ALL.len() as i32;
                Ok(Self::ALL[continuous_enum_wire!(@$strategy id, len) as usize])
            }
        }

        crate::item::ctx::ctx_free!($name);
    )*};
    (@clamp $id:expr, $len:expr) => { $id.clamp(0, $len - 1) };
    (@wrap $id:expr, $len:expr) => { $id.rem_euclid($len) };
}

continuous_enum_wire!(
    SalmonSize[clamp],
    ParrotVariant[clamp],
    MooshroomVariant[clamp],
    LlamaVariant[clamp],
    HorseVariant[wrap],
);

/// An unknown id reads as the first variant.
macro_rules! sparse_enum_wire {
    ($($name:ident),* $(,)?) => {$(
        impl Encode for $name {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                VarInt(self.id()).encode(w)
            }
        }

        impl Decode<'_> for $name {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let id = VarInt::decode(r)?.0;
                Ok(Self::ALL
                    .iter()
                    .copied()
                    .find(|variant| variant.id() == id)
                    .unwrap_or(Self::ALL[0]))
            }
        }

        crate::item::ctx::ctx_free!($name);
    )*};
}

sparse_enum_wire!(RabbitVariant, TropicalFishPattern);

impl Encode for SwingAnimation {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.kind.encode(&mut w)?;
        VarInt(self.duration.0).encode(w)
    }
}

impl Decode<'_> for SwingAnimation {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(SwingAnimation {
            kind: SwingAnimationKind::decode(r)?,
            duration: Bounded(VarInt::decode(r)?.0),
        })
    }
}
