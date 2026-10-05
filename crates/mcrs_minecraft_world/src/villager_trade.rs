use std::fmt;

use mcrs_minecraft_core::codec::{Bounded, default_true, is_default, is_true};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, rl};
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_item::enchantment::predicate::LootCondition;
use mcrs_minecraft_item::{ComponentMap, ComponentPatch};
use mcrs_minecraft_keys::{
    ContextFloatProvider, ContextIntProvider, Enchantment, Item, MapDecorationType, MobEffect,
    Potion, Structure,
};
use mcrs_minecraft_registry::{EntrySet, Id};
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor, value};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const AIR_INDEX: usize = 0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeSet {
    pub trades: EntrySet<VillagerTrade>,
    pub amount: ContextInt,
    #[serde(default, skip_serializing_if = "is_default")]
    pub allow_duplicates: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_sequence: Option<ResourceLocation>,
}

impl RegistryKey for TradeSet {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:trade_set");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VillagerTrade {
    pub wants: TradeCost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_wants: Option<TradeCost>,
    pub gives: GivenStack,
    #[serde(default = "constant::<4>", skip_serializing_if = "is_constant::<4>")]
    pub max_uses: ContextInt,
    #[serde(default = "constant::<1>", skip_serializing_if = "is_constant::<1>")]
    pub xp: ContextInt,
    #[serde(default, skip_serializing_if = "is_default")]
    pub reputation_discount: ContextFloat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_predicate: Option<LootCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub given_item_modifier: Option<ItemModifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_trade_price_enchantments: Option<EntrySet<Enchantment>>,
}

impl RegistryKey for VillagerTrade {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:villager_trade");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeCost {
    #[serde(deserialize_with = "item_id")]
    pub id: Id<Item>,
    #[serde(default = "constant::<1>", skip_serializing_if = "is_constant::<1>")]
    pub count: ContextInt,
    #[serde(default, skip_serializing_if = "is_default")]
    pub components: ComponentMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct GivenStack {
    #[serde(deserialize_with = "item_id")]
    pub id: Id<Item>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub count: Bounded<1, 99, 1>,
    #[serde(default, skip_serializing_if = "ComponentPatch::is_empty")]
    pub components: ComponentPatch,
}

impl Serialize for GivenStack {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        GivenStack::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for GivenStack {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct StackVisitor;

        impl<'de> Visitor<'de> for StackVisitor {
            type Value = GivenStack;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an item id or an item stack")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<GivenStack, E> {
                Ok(GivenStack {
                    id: item_id(value::StrDeserializer::<E>::new(id))?,
                    count: Bounded::default(),
                    components: ComponentPatch::EMPTY,
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<GivenStack, A::Error> {
                GivenStack::deserialize(value::MapAccessDeserializer::new(map))
            }
        }

        d.deserialize_any(StackVisitor)
    }
}

fn item_id<'de, D: Deserializer<'de>>(d: D) -> Result<Id<Item>, D::Error> {
    let id = Id::<Item>::deserialize(d)?;
    if id.index() == AIR_INDEX {
        return Err(D::Error::custom("Item must not be minecraft:air"));
    }
    Ok(id)
}

// chisle: only the context int providers shipped trades use are modelled
// (constant, uniform, binomial, add); another type fails the load naming it.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextInt {
    Constant(i32),
    Reference(ResourceKey<ContextIntProvider>),
    Uniform {
        min: Box<ContextInt>,
        max: Box<ContextInt>,
    },
    Binomial {
        n: Box<ContextInt>,
        p: ContextFloat,
    },
    Sum {
        inputs: Vec<ContextInt>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextFloat {
    Constant(f32),
    Reference(ResourceKey<ContextFloatProvider>),
}

impl Default for ContextFloat {
    fn default() -> Self {
        ContextFloat::Constant(0.0)
    }
}

fn constant<const N: i32>() -> ContextInt {
    ContextInt::Constant(N)
}

fn is_constant<const N: i32>(value: &ContextInt) -> bool {
    *value == ContextInt::Constant(N)
}

fn literal<T: From<i8>, const V: i8>() -> T {
    T::from(V)
}

fn is_literal<T: PartialEq + From<i8>, const V: i8>(value: &T) -> bool {
    *value == T::from(V)
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum TypedInt {
    #[serde(rename = "minecraft:constant")]
    Constant { value: i32 },
    #[serde(rename = "minecraft:uniform")]
    Uniform { min: ContextInt, max: ContextInt },
    #[serde(rename = "minecraft:binomial")]
    Binomial { n: ContextInt, p: ContextFloat },
    #[serde(rename = "minecraft:add")]
    Sum { inputs: Vec<ContextInt> },
}

impl From<TypedInt> for ContextInt {
    fn from(typed: TypedInt) -> Self {
        match typed {
            TypedInt::Constant { value } => ContextInt::Constant(value),
            TypedInt::Uniform { min, max } => ContextInt::Uniform {
                min: Box::new(min),
                max: Box::new(max),
            },
            TypedInt::Binomial { n, p } => ContextInt::Binomial { n: Box::new(n), p },
            TypedInt::Sum { inputs } => ContextInt::Sum { inputs },
        }
    }
}

impl Serialize for ContextInt {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ContextInt::Constant(value) => s.serialize_i32(*value),
            ContextInt::Reference(key) => key.serialize(s),
            ContextInt::Uniform { min, max } => {
                let mut map = s.serialize_map(Some(3))?;
                map.serialize_entry("type", "minecraft:uniform")?;
                map.serialize_entry("min", min)?;
                map.serialize_entry("max", max)?;
                map.end()
            }
            ContextInt::Binomial { n, p } => {
                let mut map = s.serialize_map(Some(3))?;
                map.serialize_entry("type", "minecraft:binomial")?;
                map.serialize_entry("n", n)?;
                map.serialize_entry("p", p)?;
                map.end()
            }
            ContextInt::Sum { inputs } => {
                let mut map = s.serialize_map(Some(2))?;
                map.serialize_entry("type", "minecraft:add")?;
                map.serialize_entry("inputs", inputs)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ContextInt {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct IntVisitor;

        impl<'de> Visitor<'de> for IntVisitor {
            type Value = ContextInt;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an int, a context int provider id or a context int provider")
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<ContextInt, E> {
                i32::try_from(value)
                    .map(ContextInt::Constant)
                    .map_err(|_| E::custom(format_args!("{value} does not fit in an int")))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<ContextInt, E> {
                i32::try_from(value)
                    .map(ContextInt::Constant)
                    .map_err(|_| E::custom(format_args!("{value} does not fit in an int")))
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<ContextInt, E> {
                ResourceLocation::read(text)
                    .map(|location| ContextInt::Reference(ResourceKey::from_location(location)))
                    .map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ContextInt, A::Error> {
                TypedInt::deserialize(value::MapAccessDeserializer::new(map)).map(ContextInt::from)
            }
        }

        d.deserialize_any(IntVisitor)
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum TypedFloat {
    #[serde(rename = "minecraft:constant")]
    Constant { value: f32 },
}

impl Serialize for ContextFloat {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ContextFloat::Constant(value) => s.serialize_f32(*value),
            ContextFloat::Reference(key) => key.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for ContextFloat {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct FloatVisitor;

        impl<'de> Visitor<'de> for FloatVisitor {
            type Value = ContextFloat;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a float, a context float provider id or a context float provider")
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<ContextFloat, E> {
                Ok(ContextFloat::Constant(value as f32))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<ContextFloat, E> {
                Ok(ContextFloat::Constant(value as f32))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<ContextFloat, E> {
                Ok(ContextFloat::Constant(value as f32))
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<ContextFloat, E> {
                ResourceLocation::read(text)
                    .map(|location| ContextFloat::Reference(ResourceKey::from_location(location)))
                    .map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ContextFloat, A::Error> {
                let TypedFloat::Constant { value } =
                    TypedFloat::deserialize(value::MapAccessDeserializer::new(map))?;
                Ok(ContextFloat::Constant(value))
            }
        }

        d.deserialize_any(FloatVisitor)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemModifier {
    One(LootFunction),
    Sequence(Vec<LootFunction>),
}

impl Serialize for ItemModifier {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ItemModifier::One(function) => function.serialize(s),
            ItemModifier::Sequence(functions) => functions.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for ItemModifier {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ModifierVisitor;

        impl<'de> Visitor<'de> for ModifierVisitor {
            type Value = ItemModifier;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a loot function or a list of loot functions")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ItemModifier, A::Error> {
                LootFunction::deserialize(value::MapAccessDeserializer::new(map))
                    .map(ItemModifier::One)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<ItemModifier, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(ItemModifier::Sequence)
            }
        }

        d.deserialize_any(ModifierVisitor)
    }
}

// chisle: only the loot function types shipped trades use are modelled; a trade
// using another type fails the load naming it. The full set arrives when loot
// tables, predicates and item modifiers load as registries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum LootFunction {
    #[serde(rename = "minecraft:discard")]
    Discard {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
    },
    #[serde(rename = "minecraft:enchant_randomly")]
    EnchantRandomly {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<EntrySet<Enchantment>>,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        only_compatible: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        include_additional_cost_component: bool,
    },
    #[serde(rename = "minecraft:enchant_with_levels")]
    EnchantWithLevels {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        levels: ContextInt,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<EntrySet<Enchantment>>,
        #[serde(default, skip_serializing_if = "is_default")]
        include_additional_cost_component: bool,
    },
    #[serde(rename = "minecraft:exploration_map")]
    ExplorationMap {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        destination: EntrySet<Structure>,
        // Absent selects minecraft:woodland_mansion.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decoration: Option<Id<MapDecorationType>>,
        #[serde(
            default = "literal::<i8, 2>",
            skip_serializing_if = "is_literal::<i8, 2>"
        )]
        zoom: i8,
        #[serde(
            default = "literal::<i32, 50>",
            skip_serializing_if = "is_literal::<i32, 50>"
        )]
        search_radius: i32,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        skip_existing_chunks: bool,
    },
    #[serde(rename = "minecraft:filtered")]
    Filtered {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        item_filter: ItemPredicate,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_pass: Option<Box<ItemModifier>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_fail: Option<Box<ItemModifier>>,
    },
    #[serde(rename = "minecraft:set_potion")]
    SetPotion {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        id: Id<Potion>,
    },
    #[serde(rename = "minecraft:set_random_dyes")]
    SetRandomDyes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        number_of_dyes: ContextInt,
    },
    #[serde(rename = "minecraft:set_random_potion")]
    SetRandomPotion {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<EntrySet<Potion>>,
    },
    #[serde(rename = "minecraft:set_stew_effect")]
    SetStewEffect {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<LootCondition>,
        #[serde(
            default,
            skip_serializing_if = "Vec::is_empty",
            deserialize_with = "distinct_effects"
        )]
        effects: Vec<StewEffect>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StewEffect {
    #[serde(rename = "type")]
    pub effect: Id<MobEffect>,
    pub duration: ContextInt,
}

fn distinct_effects<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<StewEffect>, D::Error> {
    let effects = Vec::<StewEffect>::deserialize(d)?;
    for (index, effect) in effects.iter().enumerate() {
        if effects[..index]
            .iter()
            .any(|seen| seen.effect == effect.effect)
        {
            return Err(D::Error::custom(format_args!(
                "Encountered duplicate mob effect at index {index}"
            )));
        }
    }
    Ok(effects)
}
