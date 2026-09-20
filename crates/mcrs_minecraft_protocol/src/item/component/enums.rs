use std::io::Write;

use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, STRING_ID};
use serde::{Deserialize, Serialize};

use crate::entity::DyeColor;
use crate::item::component::common::ordinal_enum;
use crate::item::component::scalar::record_codec;
use crate::item::ctx::ctx_free;
use crate::item::harness::Sample;
use crate::{Decode, Encode, VarInt};

macro_rules! enum_samples {
    ($($ty:ident),* $(,)?) => {$(
        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", STRING_ID)]
            }

            fn samples() -> Vec<Self> {
                Self::ALL.to_vec()
            }
        }
    )*};
}

/// `ByIdMap.continuous` over the ordinal with a `CLAMP` or `WRAP`
/// out-of-bounds strategy.
macro_rules! continuous_enum {
    ($name:ident [$strategy:ident] { $($variant:ident),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];
        }

        impl Encode for $name {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                VarInt(*self as i32).encode(w)
            }
        }

        impl Decode<'_> for $name {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let id = VarInt::decode(r)?.0;
                let len = Self::ALL.len() as i32;
                Ok(Self::ALL[continuous_enum!(@$strategy id, len) as usize])
            }
        }

        ctx_free!($name);
        enum_samples!($name);
    };
    (@clamp $id:expr, $len:expr) => { $id.clamp(0, $len - 1) };
    (@wrap $id:expr, $len:expr) => { $id.rem_euclid($len) };
}

/// `ByIdMap.sparse`: explicit ids, an unknown one reading as the first variant.
macro_rules! sparse_enum {
    ($name:ident { $($variant:ident = $id:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            pub const fn id(self) -> i32 {
                match self {
                    $(Self::$variant => $id),*
                }
            }
        }

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

        ctx_free!($name);
        enum_samples!($name);
    };
}

ordinal_enum! {
    Rarity { Common, Uncommon, Rare, Epic }
}

ordinal_enum! {
    MapPostProcessing { Lock, Scale }
}

ordinal_enum! {
    SwingAnimationKind { None, Whack, Stab }
}

ordinal_enum! {
    FoxVariant { Red, Snow }
}

ordinal_enum! {
    AxolotlVariant { Lucy, Wild, Gold, Cyan, Blue }
}

enum_samples!(
    Rarity,
    MapPostProcessing,
    SwingAnimationKind,
    FoxVariant,
    AxolotlVariant
);

continuous_enum!(SalmonSize[clamp] { Small, Medium, Large });
continuous_enum!(ParrotVariant[clamp] { RedBlue, Blue, Green, YellowBlue, Gray });
continuous_enum!(MooshroomVariant[clamp] { Red, Brown });
continuous_enum!(LlamaVariant[clamp] { Creamy, White, Brown, Gray });
continuous_enum!(HorseVariant[wrap] { White, Creamy, Chestnut, Brown, Black, Gray, DarkBrown });

sparse_enum!(RabbitVariant {
    Brown = 0,
    White = 1,
    Black = 2,
    WhiteSplotched = 3,
    Gold = 4,
    Salt = 5,
    Evil = 99,
});

// The id is `base | index << 8`, small base 0 and large base 1.
sparse_enum!(TropicalFishPattern {
    Kob = 0,
    Sunstreak = 256,
    Snooper = 512,
    Dasher = 768,
    Brinely = 1024,
    Spotty = 1280,
    Flopper = 1,
    Stripey = 257,
    Glitter = 513,
    Blockfish = 769,
    Betty = 1025,
    Clayfish = 1281,
});

fn whack() -> SwingAnimationKind {
    SwingAnimationKind::Whack
}

fn is_whack(kind: &SwingAnimationKind) -> bool {
    *kind == SwingAnimationKind::Whack
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct SwingAnimation {
    #[serde(rename = "type", default = "whack", skip_serializing_if = "is_whack")]
    pub kind: SwingAnimationKind,
    #[serde(default, skip_serializing_if = "is_default")]
    pub duration: Bounded<0, { i32::MAX }, 6>,
}

record_codec!(SwingAnimation);

impl Default for SwingAnimation {
    fn default() -> Self {
        SwingAnimation {
            kind: whack(),
            duration: Bounded::default(),
        }
    }
}

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

impl Sample for SwingAnimation {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !is_whack(&self.kind) {
            tags.push(("type", STRING_ID));
        }
        if !is_default(&self.duration) {
            tags.push(("duration", INT_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            SwingAnimation::default(),
            SwingAnimation {
                kind: SwingAnimationKind::Stab,
                duration: Bounded(10),
            },
            SwingAnimation {
                kind: SwingAnimationKind::None,
                duration: Bounded(0),
            },
        ]
    }
}

macro_rules! transparent_newtype {
    ($($ty:ident($inner:ident)),* $(,)?) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
        #[serde(transparent)]
        pub struct $ty(pub $inner);

        ctx_free!($ty);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                self.0.nbt_tags()
            }

            fn samples() -> Vec<Self> {
                $inner::samples().into_iter().map($ty).collect()
            }
        }
    )*};
}

transparent_newtype! {
    AttackAnimation(SwingAnimation),
    InteractAnimation(SwingAnimation),
    Dye(DyeColor),
    BaseColor(DyeColor),
    WolfCollar(DyeColor),
    TropicalFishBaseColor(DyeColor),
    TropicalFishPatternColor(DyeColor),
    CatCollar(DyeColor),
    SheepColor(DyeColor),
    ShulkerColor(DyeColor),
    CushionColor(DyeColor),
}

impl Sample for DyeColor {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", STRING_ID)]
    }

    fn samples() -> Vec<Self> {
        Self::ALL.to_vec()
    }
}
