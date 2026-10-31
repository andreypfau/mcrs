use std::fmt;

use crate::JukeboxSong;
use crate::TrimMaterial;
use crate::TrimPattern;
use crate::enchantment::EnchantmentData;
use crate::keys::DataComponentPredicateType;
use crate::keys::Item;
use crate::keys::MobEffect;
use crate::keys::Potion;
use mcrs_minecraft_block::keys::Block;
pub use mcrs_minecraft_block_predicate::state_properties::StatePropertiesPredicate;
use mcrs_minecraft_core::codec::unique_entries;
use mcrs_minecraft_entity::keys::Attribute;
use mcrs_minecraft_entity::keys::VillagerType;
use mcrs_minecraft_registries::{
    Named, Namer, Refs, Resolve, ResolveError, Resolved, Resolver, registry_value,
};
use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf, rl};
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::attribute::AttributeOperation;
use crate::component::common::{
    CompactList, EquipmentSlotGroup, MinMaxBounds, NbtPredicate, ValueMatcher, deserialize_unit,
    map_only, serialize_entries, serialize_unit,
};
use crate::component::fireworks::FireworkShape;
use crate::component::registry_ref::holder_set_tag;
use crate::component::scalar::{named_record_codec, record_codec};
use crate::harness::Sample;
use crate::harness::{entry, list_set, one_set, tag_set};
use crate::keys::DataComponentType;
use crate::patch::ComponentMap;
use mcrs_minecraft_text::IntoText;

use crate::Text;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::{Decode, Encode, Error, Result, VarInt, ensure, nested};
use std::io::Write;

pub const MAX_PARTIAL_PREDICATES: usize = 64;

macro_rules! predicate_newtype {
    ($($ty:ident($inner:ident<$mode:ty>)),* $(,)?) => {$(
        #[registry_value]
        #[derive(mcrs_minecraft_registries::FromWire)]
        #[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, Encode, Decode)]
        #[serde(transparent)]
        pub struct $ty<M: Refs = Resolved>(pub $inner<$mode>);

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

predicate_newtype!(
    CanPlaceOn(AdventureModePredicate<M>),
    CanBreak(AdventureModePredicate<M>),
    Lock(ItemPredicate<M::Nbt>),
);

mcrs_minecraft_registries::wire_as_named!(ItemPredicate);
mcrs_minecraft_registries::wire_as_named!(ComponentPredicates);

/// Every field absent: the derive would also demand a default of each reference type.
macro_rules! empty_default {
    ($($ty:ident { $($field:ident: $value:expr),* $(,)? }),* $(,)?) => {$(
        impl<M: Refs> Default for $ty<M> {
            fn default() -> Self {
                $ty { $($field: $value),* }
            }
        }
    )*};
}

/// One predicate bare, otherwise a non-empty list; the wire allows an empty
/// list.
#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Encode, Decode,
)]
#[serde(transparent)]
pub struct AdventureModePredicate<M: Refs = Resolved>(pub CompactList<BlockPredicate<M>>);

impl<M: Refs> Default for AdventureModePredicate<M> {
    fn default() -> Self {
        AdventureModePredicate(CompactList(vec![BlockPredicate::default()]))
    }
}

impl<'de> Deserialize<'de> for AdventureModePredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let predicates = CompactList::<BlockPredicate<Named>>::deserialize(d)?;
        if predicates.0.is_empty() {
            return Err(D::Error::custom("List must have contents"));
        }
        Ok(AdventureModePredicate(predicates))
    }
}

#[derive(
    Clone, Debug, PartialEq, mcrs_minecraft_registries::Resolve, mcrs_minecraft_registries::FromWire,
)]
pub struct BlockPredicate<M: Refs = Resolved> {
    pub blocks: Option<M::Set<Block>>,
    pub state: Option<StatePropertiesPredicate>,
    pub nbt: Option<NbtPredicate>,
    pub matchers: DataComponentMatchers<M>,
}

empty_default! {
    BlockPredicate {
        blocks: None,
        state: None,
        nbt: None,
        matchers: DataComponentMatchers::default(),
    },
}

impl Serialize for BlockPredicate<Named> {
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

impl<'de> Deserialize<'de> for BlockPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            #[serde(default)]
            blocks: Option<<Named as Refs>::Set<Block>>,
            #[serde(default)]
            state: Option<StatePropertiesPredicate>,
            #[serde(default)]
            nbt: Option<NbtPredicate>,
            #[serde(default)]
            components: Option<ComponentMap<Named>>,
            #[serde(default)]
            predicates: Option<ComponentPredicates<Named>>,
        }

        let repr: Repr = map_only(d)?;
        Ok(BlockPredicate {
            blocks: repr.blocks,
            state: repr.state,
            nbt: repr.nbt,
            matchers: DataComponentMatchers {
                components: repr.components.unwrap_or_default(),
                predicates: repr.predicates.unwrap_or_default(),
            },
        })
    }
}

/// The wire form of `lock` is this as one NBT tag.
#[derive(Clone, Debug, PartialEq, mcrs_minecraft_registries::Resolve)]
pub struct ItemPredicate<M: Refs = Resolved> {
    pub items: Option<M::Set<Item>>,
    pub count: MinMaxBounds<i32>,
    pub matchers: DataComponentMatchers<M>,
}

empty_default! {
    ItemPredicate {
        items: None,
        count: MinMaxBounds::ANY,
        matchers: DataComponentMatchers::default(),
    },
}

impl Serialize for ItemPredicate<Named> {
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

impl<'de> Deserialize<'de> for ItemPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            #[serde(default)]
            items: Option<<Named as Refs>::Set<Item>>,
            #[serde(default)]
            count: Option<MinMaxBounds<i32>>,
            #[serde(default)]
            components: Option<ComponentMap<Named>>,
            #[serde(default)]
            predicates: Option<ComponentPredicates<Named>>,
        }

        let repr: Repr = map_only(d)?;
        Ok(ItemPredicate {
            items: repr.items,
            count: repr.count.unwrap_or(MinMaxBounds::ANY),
            matchers: DataComponentMatchers {
                components: repr.components.unwrap_or_default(),
                predicates: repr.predicates.unwrap_or_default(),
            },
        })
    }
}

/// Exact values a component must equal and partial predicates it must
/// satisfy, both flattened into the owning predicate.
#[derive(
    Clone, Debug, PartialEq, mcrs_minecraft_registries::Resolve, mcrs_minecraft_registries::FromWire,
)]
pub struct DataComponentMatchers<M: Refs = Resolved> {
    pub components: ComponentMap<M>,
    pub predicates: ComponentPredicates<M::Nbt>,
}

empty_default! {
    DataComponentMatchers {
        components: ComponentMap::default(),
        predicates: ComponentPredicates::default(),
    },
}

impl<M: Refs> DataComponentMatchers<M> {
    pub fn is_empty(&self) -> bool {
        self.components.0.is_empty() && self.predicates.0.is_empty()
    }
}

impl DataComponentMatchers<Named> {
    fn serialize_entries<S: SerializeMap>(&self, map: &mut S) -> Result<(), S::Error> {
        if !self.components.0.is_empty() {
            map.serialize_entry("components", &self.components)?;
        }
        if !self.predicates.0.is_empty() {
            map.serialize_entry("predicates", &self.predicates)?;
        }
        Ok(())
    }
}

/// A map whose key names a predicate type, or a component type that must
/// merely be present.
#[derive(Clone, Debug, mcrs_minecraft_registries::Resolve)]
pub struct ComponentPredicates<M: Refs = Resolved>(pub Vec<ComponentPredicateEntry<M>>);

impl<M: Refs> Default for ComponentPredicates<M> {
    fn default() -> Self {
        ComponentPredicates(Vec::new())
    }
}

impl<M: Refs> PartialEq for ComponentPredicates<M> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|entry| other.0.contains(entry))
    }
}

#[derive(Clone, Debug, PartialEq, mcrs_minecraft_registries::Resolve)]
pub enum ComponentPredicateEntry<M: Refs = Resolved> {
    Typed(ComponentPredicate<M>),
    AnyValue(DataComponentType),
}

impl<M: Refs> ComponentPredicateEntry<M> {
    pub fn same_key(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Typed(a), Self::Typed(b)) => a.kind() == b.kind(),
            (Self::AnyValue(a), Self::AnyValue(b)) => a == b,
            _ => false,
        }
    }

    fn id(&self) -> ResourceLocation<&'static str> {
        match self {
            Self::Typed(predicate) => predicate.kind().location(),
            Self::AnyValue(kind) => kind.location(),
        }
    }
}

impl Serialize for ComponentPredicates<Named> {
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

impl<'de> Deserialize<'de> for ComponentPredicates<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PredicatesVisitor;

        impl<'de> Visitor<'de> for PredicatesVisitor {
            type Value = ComponentPredicates<Named>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of predicate or component ids to predicates")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries: Vec<ComponentPredicateEntry<Named>> = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    let entry = match DataComponentPredicateType::read(&key) {
                        Some(kind) => ComponentPredicateEntry::Typed(map.next_value_seed(kind)?),
                        None => match DataComponentType::read(&key) {
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
pub struct UnitMap;

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
    ($($variant:ident($ty:ty)),* $(,)?) => {
        /// A predicate as its type's codec reads it.
        #[derive(Clone, Debug, PartialEq, mcrs_minecraft_registries::Resolve)]
        pub enum ComponentPredicate<M: Refs = Resolved> {
            $($variant($ty)),*
        }

        impl<M: Refs> ComponentPredicate<M> {
            pub fn kind(&self) -> DataComponentPredicateType {
                match self {
                    $(Self::$variant(_) => DataComponentPredicateType::$variant),*
                }
            }
        }

        impl<M: Refs> Serialize for ComponentPredicate<M>
        where
            $($ty: Serialize),*
        {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self {
                    $(Self::$variant(value) => value.serialize(s)),*
                }
            }
        }

        fn read_predicate<'de, M: Refs, D: Deserializer<'de>>(
            kind: DataComponentPredicateType,
            d: D,
        ) -> Result<ComponentPredicate<M>, D::Error>
        where
            $($ty: Deserialize<'de>),*
        {
            match kind {
                $(DataComponentPredicateType::$variant => <$ty as Deserialize<'de>>::deserialize(d).map(ComponentPredicate::$variant)),*
            }
        }

        impl<'de> DeserializeSeed<'de> for DataComponentPredicateType {
            type Value = ComponentPredicate<Named>;

            fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                read_predicate::<Named, D>(self, d)
            }
        }
    };
}

predicate_types! {
    Damage(DamagePredicate),
    Enchantments(EnchantmentsPredicate<M>),
    StoredEnchantments(EnchantmentsPredicate<M>),
    PotionContents(PotionsPredicate<M>),
    CustomData(NbtPredicate),
    Container(ContainerPredicate<M>),
    BundleContents(ContainerPredicate<M>),
    FireworkExplosion(FireworkPredicate),
    Fireworks(FireworksPredicate),
    WritableBookContent(WritableBookPredicate),
    WrittenBookContent(WrittenBookPredicate<M>),
    AttributeModifiers(AttributeModifiersPredicate<M>),
    Trim(TrimPredicate<M>),
    JukeboxPlayable(JukeboxPlayablePredicate<M>),
    VillagerVariant(M::Set<VillagerType>),
}

record_codec! {
    DamagePredicate,
    MobEffectInstancePredicate,
    CollectionPredicate<P>,
    CountedPredicate<P>,
    FireworkPredicate,
    FireworksPredicate,
    WritableBookPredicate,
}

named_record_codec! {
    EnchantmentPredicate,
    PotionsPredicate,
    ContainerPredicate,
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

#[registry_value]
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EnchantmentsPredicate<M: Refs = Resolved>(pub Vec<EnchantmentPredicate<M>>);

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct EnchantmentPredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enchantments: Option<M::Set<EnchantmentData>>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub levels: MinMaxBounds<i32>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct PotionsPredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potions: Option<M::Set<Potion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<CollectionPredicate<MobEffectsPredicate<M>>>,
}

/// Effect to instance predicate, in the order read.
#[derive(Clone, Debug, PartialEq)]
pub struct MobEffectsPredicate<M: Refs = Resolved>(
    pub Vec<(M::Id<MobEffect>, MobEffectInstancePredicate)>,
);

impl<M: Refs> Default for MobEffectsPredicate<M> {
    fn default() -> Self {
        MobEffectsPredicate(Vec::new())
    }
}

impl Resolve for MobEffectsPredicate {
    type Named = MobEffectsPredicate<Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        named
            .0
            .into_iter()
            .map(|(key, predicate)| Ok((Resolve::resolve(key, ctx)?, predicate)))
            .collect::<Result<_, _>>()
            .map(MobEffectsPredicate)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        self.0
            .iter()
            .map(|(id, predicate)| Ok((id.name(names)?, predicate.clone())))
            .collect::<Result<_, _>>()
            .map(MobEffectsPredicate)
    }
}

impl Serialize for MobEffectsPredicate<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_entries(&self.0, s)
    }
}

impl<'de> Deserialize<'de> for MobEffectsPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        unique_entries(d).map(MobEffectsPredicate)
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

impl<P: Resolve> Resolve for CollectionPredicate<P> {
    type Named = CollectionPredicate<P::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        Ok(CollectionPredicate {
            contains: Option::resolve(named.contains, ctx).map_err(|e| e.at("contains"))?,
            count: Option::resolve(named.count, ctx).map_err(|e| e.at("count"))?,
            size: named.size,
        })
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        Ok(CollectionPredicate {
            contains: self.contains.name(names).map_err(|e| e.at("contains"))?,
            count: self.count.name(names).map_err(|e| e.at("count"))?,
            size: self.size,
        })
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

impl<P: Resolve> Resolve for CountedPredicate<P> {
    type Named = CountedPredicate<P::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        Ok(CountedPredicate {
            test: P::resolve(named.test, ctx).map_err(|e| e.at("test"))?,
            count: named.count,
        })
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        Ok(CountedPredicate {
            test: self.test.name(names).map_err(|e| e.at("test"))?,
            count: self.count,
        })
    }
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ContainerPredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<CollectionPredicate<ItemPredicate<M>>>,
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

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct WrittenBookPredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<CollectionPredicate<Text<M>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub generation: MinMaxBounds<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct AttributeModifiersPredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<CollectionPredicate<AttributeModifierPredicate<M>>>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct AttributeModifierPredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<M::Set<Attribute>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ResourceLocationBuf>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub amount: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<AttributeOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<EquipmentSlotGroup>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TrimPredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<M::Set<TrimMaterial>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<M::Set<TrimPattern>>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct JukeboxPlayablePredicate<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song: Option<M::Set<JukeboxSong>>,
}

empty_default! {
    EnchantmentPredicate { enchantments: None, levels: MinMaxBounds::ANY },
    PotionsPredicate { potions: None, effects: None },
    ContainerPredicate { items: None },
    WrittenBookPredicate {
        pages: None,
        author: None,
        title: None,
        generation: MinMaxBounds::ANY,
        resolved: None,
    },
    AttributeModifiersPredicate { modifiers: None },
    AttributeModifierPredicate {
        attribute: None,
        id: None,
        amount: MinMaxBounds::ANY,
        operation: None,
        slot: None,
    },
    TrimPredicate { material: None, pattern: None },
    JukeboxPlayablePredicate { song: None },
}

impl Sample for AdventureModePredicate {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
        match &self.0.0[..] {
            [only] => {
                let mut tags = vec![("", COMPOUND_ID)];
                if let Some(blocks) = &only.blocks {
                    tags.push(("blocks", holder_set_tag(blocks)));
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
                blocks: Some(one_set("stone")),
                ..Default::default()
            }])),
            AdventureModePredicate(CompactList(vec![BlockPredicate {
                blocks: Some(list_set(&["stone", "dirt"])),
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
                    blocks: Some(tag_set("logs")),
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
        use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        if let Some(items) = &self.items {
            tags.push(("items", holder_set_tag(items)));
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
                items: Some(one_set("diamond_sword")),
                count: MinMaxBounds {
                    min: Some(3),
                    max: Some(3),
                },
                ..Default::default()
            },
            ItemPredicate {
                items: Some(list_set(&["stone", "apple"])),
                count: MinMaxBounds {
                    min: Some(2),
                    max: Some(5),
                },
                matchers: sample_matchers(),
            },
            ItemPredicate {
                items: Some(tag_set("swords")),
                count: MinMaxBounds {
                    min: None,
                    max: Some(4),
                },
                ..Default::default()
            },
        ]
    }
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

#[expect(
    clippy::too_many_lines,
    reason = "builds one matcher of every kind in a single literal, so the case reads as the value it describes"
)]
fn sample_matchers() -> DataComponentMatchers {
    use crate::component::{CustomData, Damage, MaxStackSize};
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
                ComponentPredicate::VillagerVariant(list_set(&["plains", "desert"])),
                ComponentPredicate::Enchantments(EnchantmentsPredicate(vec![
                    EnchantmentPredicate {
                        enchantments: Some(one_set("sharpness")),
                        levels: at_least(2),
                    },
                    EnchantmentPredicate::default(),
                ])),
                ComponentPredicate::StoredEnchantments(EnchantmentsPredicate(vec![])),
                ComponentPredicate::PotionContents(PotionsPredicate {
                    potions: Some(one_set("healing")),
                    effects: Some(CollectionPredicate {
                        contains: Some(vec![MobEffectsPredicate(vec![(
                            entry("speed"),
                            MobEffectInstancePredicate {
                                amplifier: exactly(1),
                                duration: at_least(100),
                                ambient: Some(true),
                                visible: Some(false),
                            },
                        )])]),
                        count: Some(vec![CountedPredicate {
                            test: MobEffectsPredicate(vec![(
                                entry("haste"),
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
                            items: Some(one_set("apple")),
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
                                attribute: Some(one_set("attack_damage")),
                                id: Some(rl!("minecraft:base_attack_damage").to_buf()),
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
                    material: Some(one_set("gold")),
                    pattern: Some(list_set(&["sentry", "vex"])),
                }),
                ComponentPredicate::JukeboxPlayable(JukeboxPlayablePredicate {
                    song: Some(one_set("cat")),
                }),
            ]
            .into_iter()
            .map(ComponentPredicateEntry::Typed)
            .chain([ComponentPredicateEntry::AnyValue(
                DataComponentType::CustomName,
            )])
            .collect(),
        ),
    }
}

impl Encode for BlockPredicate<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.blocks.encode(&mut w)?;
        self.state.encode(&mut w)?;
        self.nbt.encode(&mut w)?;
        self.matchers.encode(w)
    }
}

/// A matcher map holds components that hold block predicates again.
impl Decode<'_> for BlockPredicate<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        nested(|| {
            Ok(BlockPredicate {
                blocks: Option::decode(r)?,
                state: Option::decode(r)?,
                nbt: Option::decode(r)?,
                matchers: DataComponentMatchers::decode(r)?,
            })
        })
    }
}

/// The writer is erased before recursing into the component dispatch, as
/// `ComponentPatch` does: a generic `impl Write` would otherwise gain one
/// `&mut` per level of a value that contains a value of its own kind and
/// never reach a fixed type.
impl Encode for DataComponentMatchers<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let w: &mut dyn Write = &mut w;
        self.components.encode(&mut *w)?;
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
                    mcrs_minecraft_nbt::encode_wire(predicate, &mut *w)?;
                }
                ComponentPredicateEntry::AnyValue(kind) => {
                    false.encode(&mut *w)?;
                    kind.encode(&mut *w)?;
                    mcrs_minecraft_nbt::encode_wire(&UnitMap, &mut *w)?;
                }
            }
        }
        Ok(())
    }
}

impl Decode<'_> for DataComponentMatchers<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let components = ComponentMap::decode(r)?;
        let partial = VarInt::decode(r)?.0;
        ensure!(
            (0..=MAX_PARTIAL_PREDICATES as i32).contains(&partial),
            "list of {partial} entries exceeds the maximum of {MAX_PARTIAL_PREDICATES}"
        );
        let mut predicates = ComponentPredicates::default();
        for _ in 0..partial {
            let entry = match bool::decode(r)? {
                true => {
                    let kind = DataComponentPredicateType::decode(r)?;
                    ComponentPredicateEntry::Typed(decode_predicate_wire(kind, r)?)
                }
                false => {
                    let kind = DataComponentType::decode(r)?;
                    mcrs_minecraft_nbt::decode_wire::<UnitMap>(r)?;
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

/// One network NBT tag of whatever root type the predicate's codec writes,
/// read back through that codec.
fn decode_predicate_wire(
    kind: DataComponentPredicateType,
    r: &mut &[u8],
) -> Result<ComponentPredicate<Named>> {
    mcrs_minecraft_nbt::read_wire(r, |d| kind.deserialize(&mut *d).map_err(Error::other))
}
