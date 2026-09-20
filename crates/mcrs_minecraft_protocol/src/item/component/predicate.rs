use std::fmt;
use std::io::Write;

use anyhow::{bail, ensure};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::common::{
    BlockReg, CompactList, ItemReg, MinMaxBounds, NbtPredicate, ValueMatcher, deserialize_unit,
    serialize_unit,
};
use crate::item::ctx::{DecodeCtx, EncodeCtx, decode_nbt_wire, encode_nbt_wire};
use crate::item::harness::Sample;
use crate::item::kind::{ItemComponentKind, ItemComponentValue};
use crate::item::patch::ComponentMap;
use crate::{Decode, Encode, VarInt};

pub const MAX_PARTIAL_PREDICATES: usize = 64;

macro_rules! predicate_newtype {
    ($($ty:ident($inner:ty)),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
        #[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
        #[serde(transparent)]
        pub struct $ty(pub $inner);

        impl EncodeCtx for $ty {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                self.0.encode_ctx(ctx, w)
            }
        }

        impl DecodeCtx<'_> for $ty {
            fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
                <$inner>::decode_ctx(ctx, r).map($ty)
            }
        }

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                self.0.nbt_tags()
            }

            fn samples() -> Vec<Self> {
                <$inner>::samples().into_iter().map($ty).collect()
            }
        }
    )*};
}

predicate_newtype!(
    CanPlaceOn(AdventureModePredicate),
    CanBreak(AdventureModePredicate),
    Lock(ItemPredicate),
);

/// `AdventureModePredicate.CODEC`: one predicate bare, otherwise a non-empty
/// list; the wire allows an empty list.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AdventureModePredicate(pub CompactList<BlockPredicate>);

impl Default for AdventureModePredicate {
    fn default() -> Self {
        AdventureModePredicate(CompactList(vec![BlockPredicate::default()]))
    }
}

impl<'de> Deserialize<'de> for AdventureModePredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let predicates = CompactList::<BlockPredicate>::deserialize(d)?;
        if predicates.0.is_empty() {
            return Err(D::Error::custom("List must have contents"));
        }
        Ok(AdventureModePredicate(predicates))
    }
}

impl EncodeCtx for AdventureModePredicate {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for AdventureModePredicate {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        CompactList::decode_ctx(ctx, r).map(AdventureModePredicate)
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BlockPredicate {
    pub blocks: Option<HolderSet<ResourceKey<BlockReg>>>,
    pub state: Option<StatePropertiesPredicate>,
    pub nbt: Option<NbtPredicate>,
    pub matchers: DataComponentMatchers,
}

const BLOCK_PREDICATE_FIELDS: &[&str] = &["blocks", "state", "nbt", "components", "predicates"];

impl Serialize for BlockPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        if let Some(blocks) = &self.blocks {
            map.serialize_entry("blocks", blocks)?;
        }
        if let Some(state) = &self.state {
            map.serialize_entry("state", state)?;
        }
        if let Some(nbt) = &self.nbt {
            map.serialize_entry("nbt", nbt)?;
        }
        self.matchers.serialize_entries(&mut map)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for BlockPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PredicateVisitor;

        impl<'de> Visitor<'de> for PredicateVisitor {
            type Value = BlockPredicate;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a block predicate")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut predicate = BlockPredicate::default();
                let mut matchers = MatcherFields::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "blocks" => set_once(&mut predicate.blocks, "blocks", &mut map)?,
                        "state" => set_once(&mut predicate.state, "state", &mut map)?,
                        "nbt" => set_once(&mut predicate.nbt, "nbt", &mut map)?,
                        _ => matchers.read(&key, &mut map, BLOCK_PREDICATE_FIELDS)?,
                    }
                }
                predicate.matchers = matchers.finish();
                Ok(predicate)
            }
        }

        d.deserialize_map(PredicateVisitor)
    }
}

impl EncodeCtx for BlockPredicate {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.blocks.encode_ctx(ctx, &mut w)?;
        self.state.encode_ctx(ctx, &mut w)?;
        self.nbt.encode_ctx(ctx, &mut w)?;
        self.matchers.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for BlockPredicate {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BlockPredicate {
            blocks: Option::decode_ctx(ctx, r)?,
            state: Option::decode_ctx(ctx, r)?,
            nbt: Option::decode_ctx(ctx, r)?,
            matchers: DataComponentMatchers::decode_ctx(ctx, r)?,
        })
    }
}

/// `ItemPredicate.CODEC`; the wire form of `lock` is this as one NBT tag.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ItemPredicate {
    pub items: Option<HolderSet<ResourceKey<ItemReg>>>,
    pub count: MinMaxBounds<i32>,
    pub matchers: DataComponentMatchers,
}

const ITEM_PREDICATE_FIELDS: &[&str] = &["items", "count", "components", "predicates"];

impl Serialize for ItemPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        if let Some(items) = &self.items {
            map.serialize_entry("items", items)?;
        }
        if !self.count.is_any() {
            map.serialize_entry("count", &self.count)?;
        }
        self.matchers.serialize_entries(&mut map)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ItemPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PredicateVisitor;

        impl<'de> Visitor<'de> for PredicateVisitor {
            type Value = ItemPredicate;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item predicate")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut predicate = ItemPredicate::default();
                let mut count = None;
                let mut matchers = MatcherFields::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "items" => set_once(&mut predicate.items, "items", &mut map)?,
                        "count" => set_once(&mut count, "count", &mut map)?,
                        _ => matchers.read(&key, &mut map, ITEM_PREDICATE_FIELDS)?,
                    }
                }
                predicate.count = count.unwrap_or(MinMaxBounds::ANY);
                predicate.matchers = matchers.finish();
                Ok(predicate)
            }
        }

        d.deserialize_map(PredicateVisitor)
    }
}

impl EncodeCtx for ItemPredicate {
    fn encode_ctx(&self, _: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        encode_nbt_wire(self, w)
    }
}

impl DecodeCtx<'_> for ItemPredicate {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_nbt_wire(r)
    }
}

fn set_once<'de, A: MapAccess<'de>, T: Deserialize<'de>>(
    slot: &mut Option<T>,
    field: &'static str,
    map: &mut A,
) -> Result<(), A::Error> {
    if slot.is_some() {
        return Err(A::Error::duplicate_field(field));
    }
    *slot = Some(map.next_value()?);
    Ok(())
}

/// `StatePropertiesPredicate`: property name to matcher, in the order read.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StatePropertiesPredicate(pub Vec<(String, ValueMatcher)>);

impl Serialize for StatePropertiesPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.iter().map(|(name, matcher)| (name, matcher)))
    }
}

impl<'de> Deserialize<'de> for StatePropertiesPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PropertiesVisitor;

        impl<'de> Visitor<'de> for PropertiesVisitor {
            type Value = StatePropertiesPredicate;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of property names to matchers")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut properties: Vec<(String, ValueMatcher)> = Vec::new();
                while let Some(name) = map.next_key::<String>()? {
                    if properties.iter().any(|(seen, _)| *seen == name) {
                        return Err(A::Error::custom(format_args!("Duplicate key '{name}'")));
                    }
                    properties.push((name, map.next_value()?));
                }
                Ok(StatePropertiesPredicate(properties))
            }
        }

        d.deserialize_map(PropertiesVisitor)
    }
}

impl EncodeCtx for StatePropertiesPredicate {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for StatePropertiesPredicate {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(StatePropertiesPredicate)
    }
}

/// `DataComponentMatchers`: exact values a component must equal and partial
/// predicates it must satisfy, both flattened into the owning predicate.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DataComponentMatchers {
    pub components: ComponentMap,
    pub predicates: ComponentPredicates,
}

impl DataComponentMatchers {
    pub fn is_empty(&self) -> bool {
        self.components.0.is_empty() && self.predicates.0.is_empty()
    }

    fn serialize_entries<M: SerializeMap>(&self, map: &mut M) -> Result<(), M::Error> {
        if !self.components.0.is_empty() {
            map.serialize_entry("components", &self.components)?;
        }
        if !self.predicates.0.is_empty() {
            map.serialize_entry("predicates", &self.predicates)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct MatcherFields {
    components: Option<ComponentMap>,
    predicates: Option<ComponentPredicates>,
}

impl MatcherFields {
    fn read<'de, A: MapAccess<'de>>(
        &mut self,
        key: &str,
        map: &mut A,
        fields: &'static [&'static str],
    ) -> Result<(), A::Error> {
        match key {
            "components" => set_once(&mut self.components, "components", map),
            "predicates" => set_once(&mut self.predicates, "predicates", map),
            _ => Err(A::Error::unknown_field(key, fields)),
        }
    }

    fn finish(self) -> DataComponentMatchers {
        DataComponentMatchers {
            components: self.components.unwrap_or_default(),
            predicates: self.predicates.unwrap_or_default(),
        }
    }
}

/// The writer is erased before recursing into the component dispatch, as
/// `ComponentPatch` does: a generic `impl Write` would otherwise gain one
/// `&mut` per level of a value that contains a value of its own kind and
/// never reach a fixed type.
impl EncodeCtx for DataComponentMatchers {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        let w: &mut dyn Write = &mut w;
        VarInt(self.components.0.len() as i32).encode(&mut *w)?;
        for value in &self.components.0 {
            value.kind().encode(&mut *w)?;
            value.encode_ctx_value(ctx, &mut *w)?;
        }
        let predicates = &self.predicates.0;
        ensure!(
            predicates.len() <= MAX_PARTIAL_PREDICATES,
            "list of {} entries exceeds the maximum of {MAX_PARTIAL_PREDICATES}",
            predicates.len()
        );
        VarInt(predicates.len() as i32).encode(&mut *w)?;
        for entry in predicates {
            match entry {
                ComponentPredicateEntry::Typed { kind, value } => {
                    true.encode(&mut *w)?;
                    kind.encode(&mut *w)?;
                    encode_nbt_wire(value, &mut *w)?;
                }
                ComponentPredicateEntry::AnyValue(kind) => {
                    false.encode(&mut *w)?;
                    kind.encode(&mut *w)?;
                    encode_nbt_wire(&UnitMap, &mut *w)?;
                }
            }
        }
        Ok(())
    }
}

impl DecodeCtx<'_> for DataComponentMatchers {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let exact = VarInt::decode(r)?.0;
        ensure!(exact >= 0, "attempt to decode a list with negative length");
        let mut components = ComponentMap::default();
        for _ in 0..exact {
            let kind = ItemComponentKind::decode(r)?;
            components.set_value(ItemComponentValue::decode_ctx_value(kind, ctx, r)?);
        }
        let partial = VarInt::decode(r)?.0;
        ensure!(
            (0..=MAX_PARTIAL_PREDICATES as i32).contains(&partial),
            "list of {partial} entries exceeds the maximum of {MAX_PARTIAL_PREDICATES}"
        );
        let mut predicates = ComponentPredicates::default();
        for _ in 0..partial {
            let entry = match bool::decode(r)? {
                true => ComponentPredicateEntry::Typed {
                    kind: ComponentPredicateType::decode(r)?,
                    value: decode_nbt_wire(r)?,
                },
                false => {
                    let kind = ItemComponentKind::decode(r)?;
                    decode_nbt_wire::<UnitMap>(r)?;
                    ComponentPredicateEntry::AnyValue(kind)
                }
            };
            ensure!(
                !predicates.0.iter().any(|seen| seen.same_key(&entry)),
                "duplicate data component predicate"
            );
            predicates.0.push(entry);
        }
        Ok(DataComponentMatchers {
            components,
            predicates,
        })
    }
}

/// `DataComponentPredicate.CODEC`: a map whose key names a predicate type, or
/// a component type that must merely be present.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ComponentPredicates(pub Vec<ComponentPredicateEntry>);

/// ponytail: the fifteen predicate codecs are kept as the tag they were read
/// as, so a value re-emits in its input shape and a JSON number lands in the
/// narrowest NBT tag rather than the codec's; modelling each codec removes it.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentPredicateEntry {
    Typed {
        kind: ComponentPredicateType,
        value: NbtTag,
    },
    AnyValue(ItemComponentKind),
}

impl ComponentPredicateEntry {
    fn same_key(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Typed { kind: a, .. }, Self::Typed { kind: b, .. }) => a == b,
            (Self::AnyValue(a), Self::AnyValue(b)) => a == b,
            _ => false,
        }
    }

    fn id(&self) -> ResourceLocation<&'static str> {
        match self {
            Self::Typed { kind, .. } => kind.id(),
            Self::AnyValue(kind) => kind.id(),
        }
    }
}

impl Serialize for ComponentPredicates {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for entry in &self.0 {
            match entry {
                ComponentPredicateEntry::Typed { value, .. } => {
                    map.serialize_entry(entry.id().as_str(), value)?
                }
                ComponentPredicateEntry::AnyValue(_) => {
                    map.serialize_entry(entry.id().as_str(), &UnitMap)?
                }
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ComponentPredicates {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PredicatesVisitor;

        impl<'de> Visitor<'de> for PredicatesVisitor {
            type Value = ComponentPredicates;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of predicate or component ids to predicates")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries: Vec<ComponentPredicateEntry> = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    let entry = match ComponentPredicateType::from_id(&key) {
                        Some(kind) => ComponentPredicateEntry::Typed {
                            kind,
                            value: map.next_value()?,
                        },
                        None => match ItemComponentKind::from_id(&key) {
                            Some(kind) => {
                                map.next_value::<UnitMap>()?;
                                ComponentPredicateEntry::AnyValue(kind)
                            }
                            None => {
                                return Err(A::Error::custom(format_args!(
                                    "Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_predicate_type]: {key}"
                                )));
                            }
                        },
                    };
                    if entries.iter().any(|seen| seen.same_key(&entry)) {
                        return Err(A::Error::custom(format_args!("Duplicate key '{key}'")));
                    }
                    entries.push(entry);
                }
                Ok(ComponentPredicates(entries))
            }
        }

        d.deserialize_map(PredicatesVisitor)
    }
}

/// `MapCodec.unitCodec`: `{}` written, any map read.
struct UnitMap;

impl Serialize for UnitMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_unit(s)
    }
}

impl<'de> Deserialize<'de> for UnitMap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        deserialize_unit(d).map(|()| UnitMap)
    }
}

macro_rules! predicate_types {
    ($($id:literal $name:literal : $variant:ident),* $(,)?) => {
        /// The `data_component_predicate_type` registry in registration
        /// order, which is the wire id.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum ComponentPredicateType {
            $($variant = $id),*
        }

        impl ComponentPredicateType {
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            pub const fn id(self) -> ResourceLocation<&'static str> {
                match self {
                    $(Self::$variant => ResourceLocation::new_static(concat!("minecraft:", $name))),*
                }
            }

            pub fn from_id(id: &str) -> Option<Self> {
                match id.strip_prefix("minecraft:").unwrap_or(id) {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }

        const _: () = {
            let mut position = 0;
            $(
                assert!($id == position, "a predicate type's wire id must be its position");
                position += 1;
            )*
            let _ = position;
        };
    };
}

predicate_types! {
     0 "damage"                : Damage,
     1 "enchantments"          : Enchantments,
     2 "stored_enchantments"   : StoredEnchantments,
     3 "potion_contents"       : PotionContents,
     4 "custom_data"           : CustomData,
     5 "container"             : Container,
     6 "bundle_contents"       : BundleContents,
     7 "firework_explosion"    : FireworkExplosion,
     8 "fireworks"             : Fireworks,
     9 "writable_book_content" : WritableBookContent,
    10 "written_book_content"  : WrittenBookContent,
    11 "attribute_modifiers"   : AttributeModifiers,
    12 "trim"                  : Trim,
    13 "jukebox_playable"      : JukeboxPlayable,
    14 "villager/variant"      : VillagerVariant,
}

impl Encode for ComponentPredicateType {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(*self as i32).encode(w)
    }
}

impl Decode<'_> for ComponentPredicateType {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        match usize::try_from(id).ok().and_then(|id| Self::ALL.get(id)) {
            Some(kind) => Ok(*kind),
            None => bail!("unknown data component predicate type {id}"),
        }
    }
}

impl Sample for AdventureModePredicate {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
        match &self.0.0[..] {
            [only] => {
                let mut tags = vec![("", COMPOUND_ID)];
                match only.blocks {
                    Some(HolderSet::List(_)) => tags.push(("blocks", LIST_ID)),
                    Some(_) => tags.push(("blocks", STRING_ID)),
                    None => {}
                }
                if only.state.is_some() {
                    tags.push(("state", COMPOUND_ID));
                }
                if only.nbt.is_some() {
                    tags.push(("nbt", STRING_ID));
                }
                if !only.matchers.is_empty() {
                    tags.extend([
                        ("components", COMPOUND_ID),
                        ("components.minecraft:max_stack_size", INT_ID),
                        ("components.minecraft:damage", INT_ID),
                        ("predicates", COMPOUND_ID),
                        ("predicates.minecraft:damage", COMPOUND_ID),
                        ("predicates.minecraft:custom_name", COMPOUND_ID),
                        ("predicates.minecraft:custom_data", STRING_ID),
                    ]);
                }
                tags
            }
            _ => vec![("", LIST_ID)],
        }
    }

    fn samples() -> Vec<Self> {
        vec![
            AdventureModePredicate::default(),
            AdventureModePredicate(CompactList(vec![BlockPredicate {
                blocks: Some(HolderSet::One(block("stone"))),
                ..Default::default()
            }])),
            AdventureModePredicate(CompactList(vec![BlockPredicate {
                blocks: Some(HolderSet::List(vec![block("stone"), block("dirt")])),
                state: Some(StatePropertiesPredicate(vec![
                    ("lit".into(), ValueMatcher::Exact("true".into())),
                    (
                        "age".into(),
                        ValueMatcher::Ranged {
                            min: Some("1".into()),
                            max: Some("3".into()),
                        },
                    ),
                    (
                        "open".into(),
                        ValueMatcher::Ranged {
                            min: Some("false".into()),
                            max: None,
                        },
                    ),
                    (
                        "half".into(),
                        ValueMatcher::Ranged {
                            min: None,
                            max: None,
                        },
                    ),
                ])),
                nbt: Some(NbtPredicate(sample_compound())),
                matchers: sample_matchers(),
            }])),
            AdventureModePredicate(CompactList(vec![
                BlockPredicate {
                    blocks: Some(HolderSet::Tag(ResourceLocation::minecraft("logs"))),
                    state: Some(StatePropertiesPredicate(vec![(
                        "lit".into(),
                        ValueMatcher::Exact("true".into()),
                    )])),
                    ..Default::default()
                },
                BlockPredicate {
                    nbt: Some(NbtPredicate(sample_compound())),
                    ..Default::default()
                },
            ])),
        ]
    }
}

impl Sample for ItemPredicate {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        match self.items {
            Some(HolderSet::List(_)) => tags.push(("items", LIST_ID)),
            Some(_) => tags.push(("items", STRING_ID)),
            None => {}
        }
        match (self.count.min, self.count.max) {
            (Some(min), Some(max)) if min == max => tags.push(("count", INT_ID)),
            (Some(_), _) => tags.extend([("count", COMPOUND_ID), ("count.min", INT_ID)]),
            (None, Some(_)) => tags.extend([("count", COMPOUND_ID), ("count.max", INT_ID)]),
            (None, None) => {}
        }
        if !self.matchers.is_empty() {
            tags.extend([
                ("components", COMPOUND_ID),
                ("components.minecraft:max_stack_size", INT_ID),
                ("predicates", COMPOUND_ID),
                ("predicates.minecraft:damage", COMPOUND_ID),
                ("predicates.minecraft:custom_data", STRING_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            ItemPredicate::default(),
            ItemPredicate {
                items: Some(HolderSet::One(item("diamond_sword"))),
                count: MinMaxBounds {
                    min: Some(3),
                    max: Some(3),
                },
                ..Default::default()
            },
            ItemPredicate {
                items: Some(HolderSet::List(vec![item("stone"), item("apple")])),
                count: MinMaxBounds {
                    min: Some(2),
                    max: Some(5),
                },
                matchers: sample_matchers(),
            },
            ItemPredicate {
                items: Some(HolderSet::Tag(ResourceLocation::minecraft("swords"))),
                count: MinMaxBounds {
                    min: None,
                    max: Some(4),
                },
                ..Default::default()
            },
        ]
    }
}

fn block(path: &str) -> ResourceKey<BlockReg> {
    ResourceKey::from_location(ResourceLocation::minecraft(path))
}

fn item(path: &str) -> ResourceKey<ItemReg> {
    ResourceKey::from_location(ResourceLocation::minecraft(path))
}

fn sample_compound() -> mcrs_minecraft_nbt::compound::NbtCompound {
    let mut tag = mcrs_minecraft_nbt::compound::NbtCompound::new();
    tag.put_byte("a", 1);
    tag.put_string("name", "x".into());
    tag
}

fn sample_matchers() -> DataComponentMatchers {
    use crate::item::component::{CustomData, Damage, MaxStackSize};
    use mcrs_minecraft_core::codec::Bounded;

    let mut custom = mcrs_minecraft_nbt::compound::NbtCompound::new();
    custom.put_int("x", 100_000);
    let mut damage = mcrs_minecraft_nbt::compound::NbtCompound::new();
    damage.put_component("durability", {
        let mut range = mcrs_minecraft_nbt::compound::NbtCompound::new();
        range.put_byte("min", 1);
        range
    });
    DataComponentMatchers {
        components: ComponentMap(vec![
            MaxStackSize(Bounded(16)).into(),
            CustomData(custom).into(),
            Damage(Bounded(7)).into(),
        ]),
        predicates: ComponentPredicates(vec![
            ComponentPredicateEntry::Typed {
                kind: ComponentPredicateType::Damage,
                value: NbtTag::Compound(damage),
            },
            ComponentPredicateEntry::AnyValue(ItemComponentKind::CustomName),
            ComponentPredicateEntry::Typed {
                kind: ComponentPredicateType::CustomData,
                value: NbtTag::String("{x:1}".into()),
            },
            ComponentPredicateEntry::Typed {
                kind: ComponentPredicateType::VillagerVariant,
                value: NbtTag::List(vec![
                    NbtTag::String("minecraft:plains".into()),
                    NbtTag::String("minecraft:desert".into()),
                ]),
            },
        ]),
    }
}
