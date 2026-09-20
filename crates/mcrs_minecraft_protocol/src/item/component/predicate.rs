use std::fmt;
use std::io::Write;

use anyhow::{bail, ensure};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::attribute::AttributeOperation;
use crate::item::component::common::{
    AttributeReg, BlockReg, CompactList, EnchantmentReg, EquipmentSlotGroup, ItemReg,
    JukeboxSongReg, MinMaxBounds, MobEffectReg, NbtPredicate, PotionReg, TrimMaterialReg,
    TrimPatternReg, ValueMatcher, VillagerTypeReg, deserialize_unit, serialize_unit,
};
use crate::item::component::fireworks::FireworkShape;
use crate::item::ctx::{DecodeCtx, EncodeCtx, decode_nbt_wire, encode_nbt_wire};
use crate::item::harness::Sample;
use crate::item::kind::ItemComponentKind;
use crate::item::patch::ComponentMap;
use crate::text::{IntoText, Text};
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

/// One predicate bare, otherwise a non-empty list; the wire allows an empty
/// list.
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

/// The wire form of `lock` is this as one NBT tag.
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

/// Property name to matcher, in the order read.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StatePropertiesPredicate(pub Vec<(String, ValueMatcher)>);

impl Serialize for StatePropertiesPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_entries(&self.0, s)
    }
}

impl<'de> Deserialize<'de> for StatePropertiesPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        deserialize_entries(d).map(StatePropertiesPredicate)
    }
}

/// A map kept in the order read, refusing a repeated key.
fn serialize_entries<K: Serialize, V: Serialize, S: Serializer>(
    entries: &[(K, V)],
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_map(entries.iter().map(|(key, value)| (key, value)))
}

fn deserialize_entries<'de, D, K, V>(d: D) -> Result<Vec<(K, V)>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + PartialEq + fmt::Display,
    V: Deserialize<'de>,
{
    struct EntriesVisitor<K, V>(std::marker::PhantomData<(K, V)>);

    impl<'de, K, V> Visitor<'de> for EntriesVisitor<K, V>
    where
        K: Deserialize<'de> + PartialEq + fmt::Display,
        V: Deserialize<'de>,
    {
        type Value = Vec<(K, V)>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut entries: Vec<(K, V)> = Vec::new();
            while let Some(key) = map.next_key::<K>()? {
                if entries.iter().any(|(seen, _)| *seen == key) {
                    return Err(A::Error::custom(format_args!("Duplicate key '{key}'")));
                }
                entries.push((key, map.next_value()?));
            }
            Ok(entries)
        }
    }

    d.deserialize_map(EntriesVisitor(std::marker::PhantomData))
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

/// Exact values a component must equal and partial predicates it must
/// satisfy, both flattened into the owning predicate.
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
        self.components.encode_ctx(ctx, &mut *w)?;
        let predicates = &self.predicates.0;
        ensure!(
            predicates.len() <= MAX_PARTIAL_PREDICATES,
            "list of {} entries exceeds the maximum of {MAX_PARTIAL_PREDICATES}",
            predicates.len()
        );
        VarInt(predicates.len() as i32).encode(&mut *w)?;
        for entry in predicates {
            match entry {
                ComponentPredicateEntry::Typed(predicate) => {
                    true.encode(&mut *w)?;
                    predicate.kind().encode(&mut *w)?;
                    encode_nbt_wire(predicate, &mut *w)?;
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
        let components = ComponentMap::decode_ctx(ctx, r)?;
        let partial = VarInt::decode(r)?.0;
        ensure!(
            (0..=MAX_PARTIAL_PREDICATES as i32).contains(&partial),
            "list of {partial} entries exceeds the maximum of {MAX_PARTIAL_PREDICATES}"
        );
        let mut predicates = ComponentPredicates::default();
        for _ in 0..partial {
            let entry = match bool::decode(r)? {
                true => {
                    let kind = ComponentPredicateType::decode(r)?;
                    ComponentPredicateEntry::Typed(decode_predicate_wire(kind, r)?)
                }
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

/// A map whose key names a predicate type, or a component type that must
/// merely be present.
#[derive(Clone, Debug, Default)]
pub struct ComponentPredicates(pub Vec<ComponentPredicateEntry>);

impl PartialEq for ComponentPredicates {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|entry| other.0.contains(entry))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComponentPredicateEntry {
    Typed(ComponentPredicate),
    AnyValue(ItemComponentKind),
}

impl ComponentPredicateEntry {
    fn same_key(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Typed(a), Self::Typed(b)) => a.kind() == b.kind(),
            (Self::AnyValue(a), Self::AnyValue(b)) => a == b,
            _ => false,
        }
    }

    fn id(&self) -> ResourceLocation<&'static str> {
        match self {
            Self::Typed(predicate) => predicate.kind().id(),
            Self::AnyValue(kind) => kind.id(),
        }
    }
}

/// One network NBT tag of whatever root type the predicate's codec writes,
/// read back through that codec.
fn decode_predicate_wire(
    kind: ComponentPredicateType,
    r: &mut &[u8],
) -> anyhow::Result<ComponentPredicate> {
    match r.first() {
        None => bail!("empty input for a network NBT tag"),
        Some(&mcrs_minecraft_nbt::END_ID) => bail!("a network NBT tag must not be TAG_End"),
        Some(_) => {}
    }
    let mut cursor = std::io::Cursor::new(*r);
    let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
    let value = kind.deserialize(&mut d)?;
    *r = &r[cursor.position() as usize..];
    Ok(value)
}

impl Serialize for ComponentPredicates {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for entry in &self.0 {
            match entry {
                ComponentPredicateEntry::Typed(predicate) => {
                    map.serialize_entry(entry.id().as_str(), predicate)?
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
                        Some(kind) => ComponentPredicateEntry::Typed(map.next_value_seed(kind)?),
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

/// `{}` written, any map read.
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
    ($($id:literal $name:literal : $variant:ident($ty:ty)),* $(,)?) => {
        /// The `data_component_predicate_type` registry in registration
        /// order, which is the wire id.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum ComponentPredicateType {
            $($variant = $id),*
        }

        /// A predicate as its type's codec reads it.
        #[derive(Clone, Debug, PartialEq)]
        pub enum ComponentPredicate {
            $($variant($ty)),*
        }

        impl ComponentPredicate {
            pub fn kind(&self) -> ComponentPredicateType {
                match self {
                    $(Self::$variant(_) => ComponentPredicateType::$variant),*
                }
            }
        }

        impl Serialize for ComponentPredicate {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self {
                    $(Self::$variant(value) => value.serialize(s)),*
                }
            }
        }

        impl<'de> DeserializeSeed<'de> for ComponentPredicateType {
            type Value = ComponentPredicate;

            fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                match self {
                    $(Self::$variant => <$ty as Deserialize<'de>>::deserialize(d).map(ComponentPredicate::$variant)),*
                }
            }
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
     0 "damage"                : Damage(DamagePredicate),
     1 "enchantments"          : Enchantments(EnchantmentsPredicate),
     2 "stored_enchantments"   : StoredEnchantments(EnchantmentsPredicate),
     3 "potion_contents"       : PotionContents(PotionsPredicate),
     4 "custom_data"           : CustomData(NbtPredicate),
     5 "container"             : Container(ContainerPredicate),
     6 "bundle_contents"       : BundleContents(ContainerPredicate),
     7 "firework_explosion"    : FireworkExplosion(FireworkPredicate),
     8 "fireworks"             : Fireworks(FireworksPredicate),
     9 "writable_book_content" : WritableBookContent(WritableBookPredicate),
    10 "written_book_content"  : WrittenBookContent(WrittenBookPredicate),
    11 "attribute_modifiers"   : AttributeModifiers(AttributeModifiersPredicate),
    12 "trim"                  : Trim(TrimPredicate),
    13 "jukebox_playable"      : JukeboxPlayable(JukeboxPlayablePredicate),
    14 "villager/variant"      : VillagerVariant(HolderSet<ResourceKey<VillagerTypeReg>>),
}

/// A derived record also reads a positional sequence, where vanilla reads
/// only a map. The derive is kept inherent through `remote = "Self"` and
/// reached only from a map.
macro_rules! record {
    ($($name:ident $(<$param:ident>)?),* $(,)?) => {$(
        impl<'de $(, $param: Deserialize<'de>)?> Deserialize<'de> for $name $(<$param>)? {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct MapOnly<T>(std::marker::PhantomData<T>);

                impl<'de $(, $param: Deserialize<'de>)?> Visitor<'de> for MapOnly<$name $(<$param>)?> {
                    type Value = $name $(<$param>)?;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str("a map")
                    }

                    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                        <$name $(<$param>)?>::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    }
                }

                d.deserialize_map(MapOnly(std::marker::PhantomData))
            }
        }

        impl $(<$param: Serialize>)? Serialize for $name $(<$param>)? {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                <$name $(<$param>)?>::serialize(self, s)
            }
        }
    )*};
}

record! {
    DamagePredicate,
    EnchantmentPredicate,
    PotionsPredicate,
    MobEffectInstancePredicate,
    CollectionPredicate<P>,
    CountedPredicate<P>,
    ContainerPredicate,
    FireworkPredicate,
    FireworksPredicate,
    WritableBookPredicate,
    WrittenBookPredicate,
    AttributeModifiersPredicate,
    AttributeModifierPredicate,
    TrimPredicate,
    JukeboxPlayablePredicate,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct DamagePredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub durability: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub damage: MinMaxBounds<i32>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EnchantmentsPredicate(pub Vec<EnchantmentPredicate>);

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct EnchantmentPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enchantments: Option<HolderSet<ResourceKey<EnchantmentReg>>>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub levels: MinMaxBounds<i32>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct PotionsPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potions: Option<HolderSet<ResourceKey<PotionReg>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<CollectionPredicate<MobEffectsPredicate>>,
}

/// Effect to instance predicate, in the order read.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MobEffectsPredicate(pub Vec<(ResourceKey<MobEffectReg>, MobEffectInstancePredicate)>);

impl Serialize for MobEffectsPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_entries(&self.0, s)
    }
}

impl<'de> Deserialize<'de> for MobEffectsPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        deserialize_entries(d).map(MobEffectsPredicate)
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct MobEffectInstancePredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub amplifier: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub duration: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

/// Elements that must each appear, per-element occurrence counts, and the
/// collection's size. The bounds are spelled out
/// because a defaulted field makes the derive infer `P: Default`, and an
/// inherent `deserialize` whose bounds fail silently yields to the trait's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    remote = "Self",
    deny_unknown_fields,
    bound(serialize = "P: Serialize", deserialize = "P: Deserialize<'de>")
)]
pub struct CollectionPredicate<P> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains: Option<Vec<P>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<Vec<CountedPredicate<P>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<MinMaxBounds<i32>>,
}

impl<P> Default for CollectionPredicate<P> {
    fn default() -> Self {
        CollectionPredicate {
            contains: None,
            count: None,
            size: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    remote = "Self",
    deny_unknown_fields,
    bound(serialize = "P: Serialize", deserialize = "P: Deserialize<'de>")
)]
pub struct CountedPredicate<P> {
    pub test: P,
    pub count: MinMaxBounds<i32>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ContainerPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<CollectionPredicate<ItemPredicate>>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct FireworkPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<FireworkShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_twinkle: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_trail: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct FireworksPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explosions: Option<CollectionPredicate<FireworkPredicate>>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub flight_duration: MinMaxBounds<i32>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct WritableBookPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<CollectionPredicate<String>>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct WrittenBookPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<CollectionPredicate<Text>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub generation: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct AttributeModifiersPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<CollectionPredicate<AttributeModifierPredicate>>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct AttributeModifierPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<HolderSet<ResourceKey<AttributeReg>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ResourceLocation>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub amount: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<AttributeOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<EquipmentSlotGroup>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TrimPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<HolderSet<ResourceKey<TrimMaterialReg>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<HolderSet<ResourceKey<TrimPatternReg>>>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct JukeboxPlayablePredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song: Option<HolderSet<ResourceKey<JukeboxSongReg>>>,
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
                    ]);
                    tags.extend(sample_matcher_tags());
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
            ]);
            tags.extend(sample_matcher_tags());
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

fn key<R>(path: &str) -> ResourceKey<R> {
    ResourceKey::from_location(ResourceLocation::minecraft(path))
}

fn block(path: &str) -> ResourceKey<BlockReg> {
    key(path)
}

fn item(path: &str) -> ResourceKey<ItemReg> {
    key(path)
}

fn sample_compound() -> mcrs_minecraft_nbt::compound::NbtCompound {
    let mut tag = mcrs_minecraft_nbt::compound::NbtCompound::new();
    tag.put_byte("a", 1);
    tag.put_string("name", "x".into());
    tag
}

fn sample_matcher_tags() -> Vec<(&'static str, u8)> {
    use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
    vec![
        ("predicates", COMPOUND_ID),
        ("predicates.minecraft:damage", COMPOUND_ID),
        ("predicates.minecraft:damage.durability", COMPOUND_ID),
        ("predicates.minecraft:damage.durability.min", INT_ID),
        ("predicates.minecraft:damage.damage", INT_ID),
        ("predicates.minecraft:custom_name", COMPOUND_ID),
        ("predicates.minecraft:custom_data", STRING_ID),
        ("predicates.minecraft:villager/variant", LIST_ID),
        ("predicates.minecraft:enchantments", LIST_ID),
        ("predicates.minecraft:stored_enchantments", LIST_ID),
        ("predicates.minecraft:potion_contents", COMPOUND_ID),
        ("predicates.minecraft:potion_contents.potions", STRING_ID),
        ("predicates.minecraft:potion_contents.effects", COMPOUND_ID),
        (
            "predicates.minecraft:potion_contents.effects.contains",
            LIST_ID,
        ),
        (
            "predicates.minecraft:potion_contents.effects.count",
            LIST_ID,
        ),
        (
            "predicates.minecraft:potion_contents.effects.size",
            COMPOUND_ID,
        ),
        ("predicates.minecraft:container", COMPOUND_ID),
        ("predicates.minecraft:container.items", COMPOUND_ID),
        ("predicates.minecraft:container.items.contains", LIST_ID),
        ("predicates.minecraft:container.items.size", INT_ID),
        ("predicates.minecraft:bundle_contents", COMPOUND_ID),
        ("predicates.minecraft:firework_explosion", COMPOUND_ID),
        ("predicates.minecraft:firework_explosion.shape", STRING_ID),
        (
            "predicates.minecraft:firework_explosion.has_twinkle",
            BYTE_ID,
        ),
        ("predicates.minecraft:fireworks", COMPOUND_ID),
        ("predicates.minecraft:fireworks.explosions", COMPOUND_ID),
        (
            "predicates.minecraft:fireworks.flight_duration",
            COMPOUND_ID,
        ),
        ("predicates.minecraft:fireworks.flight_duration.max", INT_ID),
        ("predicates.minecraft:writable_book_content", COMPOUND_ID),
        (
            "predicates.minecraft:writable_book_content.pages",
            COMPOUND_ID,
        ),
        (
            "predicates.minecraft:writable_book_content.pages.size",
            INT_ID,
        ),
        ("predicates.minecraft:written_book_content", COMPOUND_ID),
        (
            "predicates.minecraft:written_book_content.author",
            STRING_ID,
        ),
        (
            "predicates.minecraft:written_book_content.generation",
            COMPOUND_ID,
        ),
        (
            "predicates.minecraft:written_book_content.generation.min",
            INT_ID,
        ),
        (
            "predicates.minecraft:written_book_content.resolved",
            BYTE_ID,
        ),
        ("predicates.minecraft:attribute_modifiers", COMPOUND_ID),
        (
            "predicates.minecraft:attribute_modifiers.modifiers",
            COMPOUND_ID,
        ),
        (
            "predicates.minecraft:attribute_modifiers.modifiers.contains",
            LIST_ID,
        ),
        ("predicates.minecraft:trim", COMPOUND_ID),
        ("predicates.minecraft:trim.material", STRING_ID),
        ("predicates.minecraft:trim.pattern", LIST_ID),
        ("predicates.minecraft:jukebox_playable", COMPOUND_ID),
        ("predicates.minecraft:jukebox_playable.song", STRING_ID),
    ]
}

fn sample_matchers() -> DataComponentMatchers {
    use crate::item::component::{CustomData, Damage, MaxStackSize};
    use mcrs_minecraft_core::codec::Bounded;

    let mut custom = mcrs_minecraft_nbt::compound::NbtCompound::new();
    custom.put_int("x", 100_000);
    let mut predicate_data = mcrs_minecraft_nbt::compound::NbtCompound::new();
    predicate_data.put_int("x", 1);
    let at_least = |min: i32| MinMaxBounds {
        min: Some(min),
        max: None,
    };
    let at_most = |max: i32| MinMaxBounds {
        min: None,
        max: Some(max),
    };
    let exactly = |n: i32| MinMaxBounds {
        min: Some(n),
        max: Some(n),
    };
    DataComponentMatchers {
        components: ComponentMap(vec![
            MaxStackSize(Bounded(16)).into(),
            CustomData(custom).into(),
            Damage(Bounded(7)).into(),
        ]),
        predicates: ComponentPredicates(
            [
                ComponentPredicate::Damage(DamagePredicate {
                    durability: at_least(1),
                    damage: exactly(3),
                }),
                ComponentPredicate::CustomData(NbtPredicate(predicate_data)),
                ComponentPredicate::VillagerVariant(HolderSet::List(vec![
                    key("plains"),
                    key("desert"),
                ])),
                ComponentPredicate::Enchantments(EnchantmentsPredicate(vec![
                    EnchantmentPredicate {
                        enchantments: Some(HolderSet::One(key("sharpness"))),
                        levels: at_least(2),
                    },
                    EnchantmentPredicate::default(),
                ])),
                ComponentPredicate::StoredEnchantments(EnchantmentsPredicate(vec![])),
                ComponentPredicate::PotionContents(PotionsPredicate {
                    potions: Some(HolderSet::One(key("healing"))),
                    effects: Some(CollectionPredicate {
                        contains: Some(vec![MobEffectsPredicate(vec![(
                            key("speed"),
                            MobEffectInstancePredicate {
                                amplifier: exactly(1),
                                duration: at_least(100),
                                ambient: Some(true),
                                visible: Some(false),
                            },
                        )])]),
                        count: Some(vec![CountedPredicate {
                            test: MobEffectsPredicate(vec![(
                                key("haste"),
                                MobEffectInstancePredicate::default(),
                            )]),
                            count: at_least(1),
                        }]),
                        size: Some(MinMaxBounds::ANY),
                    }),
                }),
                ComponentPredicate::Container(ContainerPredicate {
                    items: Some(CollectionPredicate {
                        contains: Some(vec![ItemPredicate {
                            items: Some(HolderSet::One(item("apple"))),
                            count: at_least(2),
                            ..Default::default()
                        }]),
                        count: None,
                        size: Some(exactly(3)),
                    }),
                }),
                ComponentPredicate::BundleContents(ContainerPredicate::default()),
                ComponentPredicate::FireworkExplosion(FireworkPredicate {
                    shape: Some(FireworkShape::Star),
                    has_twinkle: Some(true),
                    has_trail: None,
                }),
                ComponentPredicate::Fireworks(FireworksPredicate {
                    explosions: Some(CollectionPredicate {
                        contains: Some(vec![FireworkPredicate {
                            shape: Some(FireworkShape::Burst),
                            has_twinkle: None,
                            has_trail: Some(false),
                        }]),
                        count: None,
                        size: Some(at_least(1)),
                    }),
                    flight_duration: at_most(2),
                }),
                ComponentPredicate::WritableBookContent(WritableBookPredicate {
                    pages: Some(CollectionPredicate {
                        contains: Some(vec!["hello".into()]),
                        count: None,
                        size: Some(exactly(1)),
                    }),
                }),
                ComponentPredicate::WrittenBookContent(WrittenBookPredicate {
                    pages: Some(CollectionPredicate {
                        contains: Some(vec![Text::text("a"), "b".bold()]),
                        count: None,
                        size: None,
                    }),
                    author: Some("me".into()),
                    title: Some("t".into()),
                    generation: at_least(1),
                    resolved: Some(true),
                }),
                ComponentPredicate::AttributeModifiers(AttributeModifiersPredicate {
                    modifiers: Some(CollectionPredicate {
                        contains: Some(vec![
                            AttributeModifierPredicate {
                                attribute: Some(HolderSet::One(key("attack_damage"))),
                                id: Some(ResourceLocation::minecraft("base_attack_damage")),
                                amount: MinMaxBounds {
                                    min: Some(1.5),
                                    max: None,
                                },
                                operation: Some(AttributeOperation::AddValue),
                                slot: Some(EquipmentSlotGroup::Mainhand),
                            },
                            AttributeModifierPredicate {
                                amount: MinMaxBounds {
                                    min: Some(2.0),
                                    max: Some(2.0),
                                },
                                ..Default::default()
                            },
                        ]),
                        count: None,
                        size: None,
                    }),
                }),
                ComponentPredicate::Trim(TrimPredicate {
                    material: Some(HolderSet::One(key("gold"))),
                    pattern: Some(HolderSet::List(vec![key("sentry"), key("vex")])),
                }),
                ComponentPredicate::JukeboxPlayable(JukeboxPlayablePredicate {
                    song: Some(HolderSet::One(key("cat"))),
                }),
            ]
            .into_iter()
            .map(ComponentPredicateEntry::Typed)
            .chain([ComponentPredicateEntry::AnyValue(
                ItemComponentKind::CustomName,
            )])
            .collect(),
        ),
    }
}
