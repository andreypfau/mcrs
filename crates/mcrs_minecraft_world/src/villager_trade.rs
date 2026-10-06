use std::fmt;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_item::{ComponentMap, ComponentPatch};
use mcrs_minecraft_loot::number::{FloatExpression, IntExpression, is_zero_holder, zero_holder};
use mcrs_minecraft_loot::{LootCondition, LootItemFunction};
use mcrs_minecraft_registry::{Holder, HolderSet, Id};
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const AIR_INDEX: usize = 0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeSet {
    pub trades: HolderSet<crate::villager_trade::VillagerTrade>,
    pub amount: Holder<IntExpression>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub allow_duplicates: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_sequence: Option<ResourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VillagerTrade {
    pub wants: TradeCost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_wants: Option<TradeCost>,
    pub gives: GivenStack,
    #[serde(default = "constant::<4>", skip_serializing_if = "is_constant::<4>")]
    pub max_uses: Holder<IntExpression>,
    #[serde(default = "constant::<1>", skip_serializing_if = "is_constant::<1>")]
    pub xp: Holder<IntExpression>,
    #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
    pub reputation_discount: Holder<FloatExpression>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_predicate: Option<Holder<LootCondition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub given_item_modifier: Option<Holder<LootItemFunction>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_trade_price_enchantments: Option<HolderSet<EnchantmentData>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeCost {
    #[serde(deserialize_with = "item_id")]
    pub id: Id<Item>,
    #[serde(default = "constant::<1>", skip_serializing_if = "is_constant::<1>")]
    pub count: Holder<IntExpression>,
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

fn constant<const N: i32>() -> Holder<IntExpression> {
    Holder::Direct(IntExpression::Constant(N))
}

fn is_constant<const N: i32>(value: &Holder<IntExpression>) -> bool {
    *value == constant::<N>()
}
