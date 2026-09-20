use std::fmt;
use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec::{Bounded, NonNegativeInt, Validate, float_value, int_value};
use mcrs_minecraft_core::{HolderSet, ResourceKey, validated};
use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, INT_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::common::{
    BannerPatternReg, BlockReg, BlockTransformerReg, DamageTypeReg, EnchantmentReg, EntityTypeReg,
    ItemReg, MobEffectReg, lenient,
};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::{Decode, Encode, VarInt};

/// `RegistryFixedCodec` / `ByteBufCodecs.holderRegistry`: an id string, one
/// raw VarInt on the wire, never inline.
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

        impl $crate::item::ctx::EncodeCtx for $ty {
            fn encode_ctx(
                &self,
                ctx: &dyn mcrs_minecraft_registry::RegistryLookup,
                w: impl std::io::Write,
            ) -> anyhow::Result<()> {
                $crate::item::ctx::EncodeCtx::encode_ctx(&self.0, ctx, w)
            }
        }

        impl $crate::item::ctx::DecodeCtx<'_> for $ty {
            fn decode_ctx(
                ctx: &dyn mcrs_minecraft_registry::RegistryLookup,
                r: &mut &[u8],
            ) -> anyhow::Result<Self> {
                <mcrs_minecraft_core::ResourceKey<$registry> as $crate::item::ctx::DecodeCtx>::decode_ctx(ctx, r)
                    .map($ty)
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

/// `ItemEnchantments.CODEC`: a map of enchantment id to level in 1..=255,
/// kept in read order because vanilla's own order is hash order.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Enchantments(pub Vec<(ResourceKey<EnchantmentReg>, i32)>);

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
                let mut entries: Vec<(ResourceKey<EnchantmentReg>, i32)> =
                    Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((key, Level(level))) =
                    map.next_entry::<ResourceKey<EnchantmentReg>, Level>()?
                {
                    if entries.iter().any(|(k, _)| *k == key) {
                        return Err(A::Error::custom(format_args!("Duplicate key: {key}")));
                    }
                    entries.push((key, level));
                }
                Ok(Enchantments(entries))
            }
        }

        d.deserialize_map(EnchantmentsVisitor)
    }
}

impl EncodeCtx for Enchantments {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0.len() as i32).encode(&mut w)?;
        for (enchantment, level) in &self.0 {
            enchantment.encode_ctx(ctx, &mut w)?;
            VarInt(*level).encode(&mut w)?;
        }
        Ok(())
    }
}

impl DecodeCtx<'_> for Enchantments {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode a map with negative length");
        let mut entries: Vec<(ResourceKey<EnchantmentReg>, i32)> =
            Vec::with_capacity((len as usize).min(r.len()));
        for _ in 0..len {
            let enchantment = ResourceKey::decode_ctx(ctx, r)?;
            let level = check_level(VarInt::decode(r)?.0).map_err(anyhow::Error::msg)?;
            ensure!(
                entries.iter().all(|(k, _)| *k != enchantment),
                "Duplicate key: {enchantment}"
            );
            entries.push((enchantment, level));
        }
        Ok(Enchantments(entries))
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

impl EncodeCtx for StoredEnchantments {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for StoredEnchantments {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Enchantments::decode_ctx(ctx, r).map(StoredEnchantments)
    }
}

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

impl EncodeCtx for DamageResistant {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.types.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for DamageResistant {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(DamageResistant {
            types: HolderSet::decode_ctx(ctx, r)?,
        })
    }
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
        skip_serializing_if = "is_default_mining_speed"
    )]
    pub default_mining_speed: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub damage_per_block: NonNegativeInt,
    #[serde(
        default = "mcrs_minecraft_core::codec::default_true",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub can_destroy_blocks_in_creative: bool,
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
            Some(speed) if speed <= 0.0 => Err(format!("Value must be positive: {speed:?}")),
            _ => Ok(()),
        }
    }
}

impl EncodeCtx for ToolRule {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.blocks.encode_ctx(ctx, &mut w)?;
        self.speed.encode(&mut w)?;
        self.correct_for_drops.encode(w)
    }
}

impl DecodeCtx<'_> for ToolRule {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ToolRule {
            blocks: HolderSet::decode_ctx(ctx, r)?,
            speed: Option::decode(r)?,
            correct_for_drops: Option::decode(r)?,
        })
    }
}

impl EncodeCtx for Tool {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.rules.encode_ctx(ctx, &mut w)?;
        self.default_mining_speed.encode(&mut w)?;
        VarInt(self.damage_per_block.0).encode(&mut w)?;
        self.can_destroy_blocks_in_creative.encode(w)
    }
}

impl DecodeCtx<'_> for Tool {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Tool {
            rules: Vec::decode_ctx(ctx, r)?,
            default_mining_speed: f32::decode(r)?,
            damage_per_block: Bounded(VarInt::decode(r)?.0),
            can_destroy_blocks_in_creative: bool::decode(r)?,
        })
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

impl EncodeCtx for Repairable {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.items.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Repairable {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Repairable {
            items: HolderSet::decode_ctx(ctx, r)?,
        })
    }
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
    if (0.0..=MAX_MOB_VISIBILITY).contains(&visibility) {
        Ok(visibility)
    } else {
        Err(D::Error::custom(format_args!(
            "Value must be within range [0.0;{MAX_MOB_VISIBILITY:?}]: {visibility:?}"
        )))
    }
}

impl EncodeCtx for MobVisibility {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.targeting_entity_types.encode_ctx(ctx, &mut w)?;
        self.visibility.encode(w)
    }
}

impl DecodeCtx<'_> for MobVisibility {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(MobVisibility {
            targeting_entity_types: HolderSet::decode_ctx(ctx, r)?,
            visibility: f32::decode(r)?,
        })
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

impl EncodeCtx for ProvidesBannerPatterns {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for ProvidesBannerPatterns {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        HolderSet::decode_ctx(ctx, r).map(ProvidesBannerPatterns)
    }
}

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

fn lenient_duration<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    struct Duration(i32);

    impl<'de> Deserialize<'de> for Duration {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            int_value(d).map(Duration)
        }
    }

    let duration: Option<Duration> = lenient(d)?;
    Ok(duration.map_or(DEFAULT_STEW_DURATION, |Duration(v)| v))
}

impl EncodeCtx for StewEntry {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode_ctx(ctx, &mut w)?;
        VarInt(self.duration).encode(w)
    }
}

impl DecodeCtx<'_> for StewEntry {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(StewEntry {
            id: ResourceKey::decode_ctx(ctx, r)?,
            duration: VarInt::decode(r)?.0,
        })
    }
}

impl EncodeCtx for SuspiciousStewEffects {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for SuspiciousStewEffects {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(SuspiciousStewEffects)
    }
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
