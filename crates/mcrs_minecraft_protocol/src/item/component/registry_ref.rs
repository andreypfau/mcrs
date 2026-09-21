use std::borrow::Cow;
use std::fmt;

use mcrs_minecraft_core::codec::{Bounded, NonNegativeInt, Validate, float_value, int_value};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation, validated};
use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, INT_ID, LIST_ID, STRING_ID};
use serde::de::{Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::common::{
    BannerPatternReg, BlockReg, BlockTransformerReg, DamageTypeReg, EnchantmentReg, EntityTypeReg,
    ItemReg, MobEffectReg,
};
use crate::item::harness::Sample;

/// An id string, one raw VarInt on the wire, never inline.
macro_rules! registry_key_component {
    ($($ty:ident($registry:ident) [$($sample:literal),+]),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub mcrs_minecraft_core::ResourceKey<$registry>);

        impl $ty {
            pub fn minecraft(path: &str) -> Self {
                $ty(mcrs_minecraft_core::ResourceKey::from_location(
                    mcrs_minecraft_core::ResourceLocation::minecraft(path),
                ))
            }
        }

        impl $crate::item::harness::Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", mcrs_minecraft_nbt::STRING_ID)]
            }

            fn samples() -> Vec<Self> {
                vec![$($ty::minecraft($sample)),+]
            }
        }
    )*};
}
pub(crate) use registry_key_component;

registry_key_component! {
    DamageTypeRef(DamageTypeReg) ["in_fire", "lava"],
    BlockTransformerRef(BlockTransformerReg) ["axe", "shovel"],
}

/// JSON's `null` reads as an absent field.
macro_rules! null_as_default {
    ($($name:ident: $ty:ty = $default:expr;)*) => {$(
        fn $name<'de, D: serde::Deserializer<'de>>(d: D) -> Result<$ty, D::Error> {
            Ok(<Option<$ty> as serde::Deserialize>::deserialize(d)?.unwrap_or_else(|| $default))
        }
    )*};
}
pub(crate) use null_as_default;

/// Enchantment id to level in 1..=255, kept in read order because vanilla's
/// own order is hash order.
#[derive(Clone, Debug, Eq, Default)]
pub struct Enchantments(pub Vec<(ResourceKey<EnchantmentReg>, i32)>);

impl Enchantments {
    /// Zero when absent.
    pub fn level(&self, enchantment: &ResourceLocation) -> i32 {
        self.0
            .iter()
            .find(|(key, _)| key.location() == enchantment)
            .map_or(0, |(_, level)| *level)
    }
}

impl PartialEq for Enchantments {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|entry| other.0.contains(entry))
    }
}

pub const MIN_ENCHANTMENT_LEVEL: i32 = 1;
pub const MAX_ENCHANTMENT_LEVEL: i32 = 255;

fn check_level(level: i32) -> Result<i32, String> {
    if (MIN_ENCHANTMENT_LEVEL..=MAX_ENCHANTMENT_LEVEL).contains(&level) {
        Ok(level)
    } else {
        Err(format!(
            "Value {level} outside of range [{MIN_ENCHANTMENT_LEVEL}:{MAX_ENCHANTMENT_LEVEL}]"
        ))
    }
}

impl Serialize for Enchantments {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (enchantment, level) in &self.0 {
            map.serialize_entry(enchantment, level)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Enchantments {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Level(i32);

        impl<'de> Deserialize<'de> for Level {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                check_level(int_value(d)?)
                    .map(Level)
                    .map_err(D::Error::custom)
            }
        }

        struct EnchantmentsVisitor;

        impl<'de> Visitor<'de> for EnchantmentsVisitor {
            type Value = Enchantments;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of enchantment ids to levels")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries: Vec<(Cow<'de, str>, ResourceKey<EnchantmentReg>, i32)> =
                    Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((raw, Level(level))) = map.next_entry::<Cow<'de, str>, Level>()? {
                    if let Some(entry) = entries.iter_mut().find(|(r, _, _)| *r == raw) {
                        entry.2 = level;
                        continue;
                    }
                    let key = ResourceKey::from_location(
                        ResourceLocation::read(&raw).map_err(A::Error::custom)?,
                    );
                    if entries.iter().any(|(_, k, _)| *k == key) {
                        return Err(A::Error::custom(format_args!(
                            "Duplicate entry for key: {key}"
                        )));
                    }
                    entries.push((raw, key, level));
                }
                Ok(Enchantments(
                    entries
                        .into_iter()
                        .map(|(_, k, level)| (k, level))
                        .collect(),
                ))
            }
        }

        d.deserialize_map(EnchantmentsVisitor)
    }
}

impl Sample for Enchantments {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !self.0.is_empty() {
            tags.push(("minecraft:sharpness", INT_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Enchantments::default(),
            Enchantments(vec![(minecraft("sharpness"), 5)]),
            Enchantments(vec![
                (minecraft("sharpness"), 1),
                (minecraft("unbreaking"), 255),
            ]),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StoredEnchantments(pub Enchantments);

impl Sample for StoredEnchantments {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        self.0.nbt_tags()
    }

    fn samples() -> Vec<Self> {
        Enchantments::samples()
            .into_iter()
            .map(StoredEnchantments)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageResistant {
    pub types: HolderSet<ResourceKey<DamageTypeReg>>,
}

impl Sample for DamageResistant {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", COMPOUND_ID), ("types", holder_set_tag(&self.types))]
    }

    fn samples() -> Vec<Self> {
        vec![
            DamageResistant {
                types: HolderSet::Tag(mcrs_minecraft_core::ResourceLocation::minecraft("is_fire")),
            },
            DamageResistant {
                types: HolderSet::One(minecraft("lava")),
            },
            DamageResistant {
                types: HolderSet::List(vec![minecraft("in_fire"), minecraft("lava")]),
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub rules: Vec<ToolRule>,
    #[serde(
        default = "default_mining_speed",
        deserialize_with = "mining_speed_or_default",
        skip_serializing_if = "is_default_mining_speed"
    )]
    pub default_mining_speed: f32,
    #[serde(
        default = "one",
        deserialize_with = "damage_per_block_or_default",
        skip_serializing_if = "is_one"
    )]
    pub damage_per_block: NonNegativeInt,
    #[serde(
        default = "mcrs_minecraft_core::codec::default_true",
        deserialize_with = "creative_or_default",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub can_destroy_blocks_in_creative: bool,
}

null_as_default! {
    mining_speed_or_default: f32 = default_mining_speed();
    damage_per_block_or_default: NonNegativeInt = one();
    creative_or_default: bool = true;
}

fn default_mining_speed() -> f32 {
    1.0
}

fn is_default_mining_speed(speed: &f32) -> bool {
    speed.to_bits() == 1.0f32.to_bits()
}

fn one() -> NonNegativeInt {
    Bounded(1)
}

fn is_one(value: &NonNegativeInt) -> bool {
    value.0 == 1
}

impl Default for Tool {
    fn default() -> Self {
        Tool {
            rules: Vec::new(),
            default_mining_speed: 1.0,
            damage_per_block: one(),
            can_destroy_blocks_in_creative: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, remote = "Self")]
pub struct ToolRule {
    pub blocks: HolderSet<ResourceKey<BlockReg>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correct_for_drops: Option<bool>,
}

validated!(ToolRule);

impl Validate for ToolRule {
    fn validate(&self) -> Result<(), String> {
        match self.speed {
            Some(speed) if !(speed > 0.0 && speed <= f32::MAX) => {
                Err(format!("Value must be positive: {speed:?}"))
            }
            _ => Ok(()),
        }
    }
}

impl Sample for Tool {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID), ("rules", LIST_ID)];
        if !is_default_mining_speed(&self.default_mining_speed) {
            tags.push(("default_mining_speed", FLOAT_ID));
        }
        if !is_one(&self.damage_per_block) {
            tags.push(("damage_per_block", INT_ID));
        }
        if !self.can_destroy_blocks_in_creative {
            tags.push((
                "can_destroy_blocks_in_creative",
                mcrs_minecraft_nbt::BYTE_ID,
            ));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Tool::default(),
            Tool {
                rules: vec![
                    ToolRule {
                        blocks: HolderSet::Tag(mcrs_minecraft_core::ResourceLocation::minecraft(
                            "mineable/pickaxe",
                        )),
                        speed: Some(8.0),
                        correct_for_drops: Some(true),
                    },
                    ToolRule {
                        blocks: HolderSet::List(vec![minecraft("stone"), minecraft("dirt")]),
                        speed: None,
                        correct_for_drops: None,
                    },
                    ToolRule {
                        blocks: HolderSet::One(minecraft("stone")),
                        speed: None,
                        correct_for_drops: Some(false),
                    },
                ],
                default_mining_speed: 4.0,
                damage_per_block: Bounded(2),
                can_destroy_blocks_in_creative: false,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repairable {
    pub items: HolderSet<ResourceKey<ItemReg>>,
}

impl Sample for Repairable {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", COMPOUND_ID), ("items", holder_set_tag(&self.items))]
    }

    fn samples() -> Vec<Self> {
        vec![
            Repairable {
                items: HolderSet::Tag(mcrs_minecraft_core::ResourceLocation::minecraft("planks")),
            },
            Repairable {
                items: HolderSet::One(minecraft("diamond_sword")),
            },
            Repairable {
                items: HolderSet::List(vec![minecraft("stone"), minecraft("apple")]),
            },
        ]
    }
}

pub const MAX_MOB_VISIBILITY: f32 = 10.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobVisibility {
    pub targeting_entity_types: HolderSet<ResourceKey<EntityTypeReg>>,
    #[serde(deserialize_with = "visibility_range")]
    pub visibility: f32,
}

fn visibility_range<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let visibility = float_value(d)?;
    if visibility.total_cmp(&0.0).is_ge() && visibility.total_cmp(&MAX_MOB_VISIBILITY).is_le() {
        Ok(visibility)
    } else {
        Err(D::Error::custom(format_args!(
            "Value must be within range [0.0;{MAX_MOB_VISIBILITY:?}]: {visibility:?}"
        )))
    }
}

impl Sample for MobVisibility {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![
            ("", COMPOUND_ID),
            (
                "targeting_entity_types",
                holder_set_tag(&self.targeting_entity_types),
            ),
            ("visibility", FLOAT_ID),
        ]
    }

    fn samples() -> Vec<Self> {
        vec![
            MobVisibility {
                targeting_entity_types: HolderSet::Tag(
                    mcrs_minecraft_core::ResourceLocation::minecraft("skeletons"),
                ),
                visibility: 0.0,
            },
            MobVisibility {
                targeting_entity_types: HolderSet::One(minecraft("zombie")),
                visibility: 0.5,
            },
            MobVisibility {
                targeting_entity_types: HolderSet::List(vec![
                    minecraft("zombie"),
                    minecraft("pig"),
                ]),
                visibility: MAX_MOB_VISIBILITY,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProvidesBannerPatterns(pub HolderSet<ResourceKey<BannerPatternReg>>);

impl Sample for ProvidesBannerPatterns {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", holder_set_tag(&self.0))]
    }

    fn samples() -> Vec<Self> {
        vec![
            ProvidesBannerPatterns(HolderSet::Tag(
                mcrs_minecraft_core::ResourceLocation::minecraft("pattern_item/globe"),
            )),
            ProvidesBannerPatterns(HolderSet::One(minecraft("globe"))),
            ProvidesBannerPatterns(HolderSet::List(vec![
                minecraft("globe"),
                minecraft("creeper"),
            ])),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SuspiciousStewEffects(pub Vec<StewEntry>);

pub const DEFAULT_STEW_DURATION: i32 = 160;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StewEntry {
    pub id: ResourceKey<MobEffectReg>,
    #[serde(
        default = "default_stew_duration",
        deserialize_with = "lenient_duration",
        skip_serializing_if = "is_default_stew_duration"
    )]
    pub duration: i32,
}

fn default_stew_duration() -> i32 {
    DEFAULT_STEW_DURATION
}

fn is_default_stew_duration(duration: &i32) -> bool {
    *duration == DEFAULT_STEW_DURATION
}

/// Anything that is not a number reads as the default. Sequences and maps are
/// drained so a streaming input is left at the next field.
fn lenient_duration<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    struct LenientDuration {
        human_readable: bool,
    }

    impl<'de> Visitor<'de> for LenientDuration {
        type Value = i32;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a duration")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<i32, E> {
            Ok(v as i32)
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<i32, E> {
            Ok(v as i32)
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<i32, E> {
            if self.human_readable {
                int_value(serde::de::value::F64Deserializer::<E>::new(v))
            } else {
                Ok(v as i32)
            }
        }

        fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<i32, E> {
            Ok(DEFAULT_STEW_DURATION)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<i32, E> {
            Ok(DEFAULT_STEW_DURATION)
        }

        fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<i32, E> {
            Ok(DEFAULT_STEW_DURATION)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<i32, A::Error> {
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            Ok(DEFAULT_STEW_DURATION)
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<i32, A::Error> {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok(DEFAULT_STEW_DURATION)
        }
    }

    let human_readable = d.is_human_readable();
    d.deserialize_any(LenientDuration { human_readable })
}

impl Sample for SuspiciousStewEffects {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            SuspiciousStewEffects::default(),
            SuspiciousStewEffects(vec![StewEntry {
                id: minecraft("speed"),
                duration: DEFAULT_STEW_DURATION,
            }]),
            SuspiciousStewEffects(vec![
                StewEntry {
                    id: minecraft("speed"),
                    duration: 1,
                },
                StewEntry {
                    id: minecraft("slowness"),
                    duration: 200,
                },
            ]),
        ]
    }
}

fn minecraft<R>(path: &str) -> ResourceKey<R> {
    ResourceKey::from_location(mcrs_minecraft_core::ResourceLocation::minecraft(path))
}

fn holder_set_tag<T>(set: &HolderSet<T>) -> u8 {
    match set {
        HolderSet::List(entries) if entries.len() != 1 => LIST_ID,
        _ => STRING_ID,
    }
}
