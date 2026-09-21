use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, STRING_ID};
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::common::ordinal_enum;
use crate::component::scalar::record_codec;
use crate::harness::Sample;

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

        enum_samples!($name);
    };
}

/// Explicit wire ids.
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

#[allow(clippy::derivable_impls)]
impl Default for SwingAnimationKind {
    fn default() -> Self {
        SwingAnimationKind::Whack
    }
}

/// A JSON `null` reads as a missing key.
fn null_is_absent<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(
    d: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct SwingAnimation {
    #[serde(
        rename = "type",
        default,
        deserialize_with = "null_is_absent",
        skip_serializing_if = "is_default"
    )]
    pub kind: SwingAnimationKind,
    #[serde(
        default,
        deserialize_with = "null_is_absent",
        skip_serializing_if = "is_default"
    )]
    pub duration: Bounded<0, { i32::MAX }, 6>,
}

record_codec!(SwingAnimation);

impl Sample for SwingAnimation {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !is_default(&self.kind) {
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
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub $inner);

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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DyeColor {
    White,
    Orange,
    Magenta,
    LightBlue,
    Yellow,
    Lime,
    Pink,
    Gray,
    LightGray,
    Cyan,
    Purple,
    Blue,
    Brown,
    Green,
    Red,
    Black,
}

impl DyeColor {
    pub const ALL: [Self; 16] = [
        Self::White,
        Self::Orange,
        Self::Magenta,
        Self::LightBlue,
        Self::Yellow,
        Self::Lime,
        Self::Pink,
        Self::Gray,
        Self::LightGray,
        Self::Cyan,
        Self::Purple,
        Self::Blue,
        Self::Brown,
        Self::Green,
        Self::Red,
        Self::Black,
    ];
}

impl Sample for DyeColor {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", STRING_ID)]
    }

    fn samples() -> Vec<Self> {
        Self::ALL.to_vec()
    }
}
