use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, STRING_ID};
use serde::{Deserialize, Serialize};

use crate::component::common::{ordinal_enum, transparent_newtype};
use crate::component::registry_ref::null_as_default;
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

ordinal_enum! {
    SalmonSize { Small, Medium, Large }
}

ordinal_enum! {
    ParrotVariant { RedBlue, Blue, Green, YellowBlue, Gray }
}

ordinal_enum! {
    MooshroomVariant { Red, Brown }
}

ordinal_enum! {
    LlamaVariant { Creamy, White, Brown, Gray }
}

ordinal_enum! {
    HorseVariant { White, Creamy, Chestnut, Brown, Black, Gray, DarkBrown }
}

ordinal_enum! {
    DyeColor {
        White, Orange, Magenta, LightBlue, Yellow, Lime, Pink, Gray,
        LightGray, Cyan, Purple, Blue, Brown, Green, Red, Black,
    }
}

enum_samples!(
    Rarity,
    MapPostProcessing,
    SwingAnimationKind,
    FoxVariant,
    AxolotlVariant,
    SalmonSize,
    ParrotVariant,
    MooshroomVariant,
    LlamaVariant,
    HorseVariant,
    DyeColor
);

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

null_as_default! {
    kind_or_default: SwingAnimationKind = Default::default();
    duration_or_default: Bounded<0, { i32::MAX }, 6> = Default::default();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct SwingAnimation {
    #[serde(
        rename = "type",
        default,
        deserialize_with = "kind_or_default",
        skip_serializing_if = "is_default"
    )]
    pub kind: SwingAnimationKind,
    #[serde(
        default,
        deserialize_with = "duration_or_default",
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

transparent_newtype! {
    AttackAnimation(SwingAnimation) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    InteractAnimation(SwingAnimation) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    Dye(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    BaseColor(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    WolfCollar(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    TropicalFishBaseColor(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    TropicalFishPatternColor(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    CatCollar(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    SheepColor(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    ShulkerColor(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
    CushionColor(DyeColor) => [Clone, Copy, Debug, PartialEq, Eq, Hash],
}
