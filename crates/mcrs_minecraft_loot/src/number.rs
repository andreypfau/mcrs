use std::fmt;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{Number, int_value};
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_environment::attribute::spec::{AttributeType, attribute};
use mcrs_minecraft_environment::keys::EnvironmentAttribute;
use mcrs_minecraft_item::enchantment::value::LevelBasedValue;
use mcrs_minecraft_item::keys::{ContextFloatProviderType, ContextIntProviderType};
use mcrs_minecraft_item::loot::{ContextFloatProvider, ContextIntProvider};
use mcrs_minecraft_registry::{Holder, HolderList};
use mcrs_minecraft_value_provider::Weighted;
use serde::de::{DeserializeOwned, Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::buffer::MapEntries;
use crate::condition::LootCondition;
use crate::provider::ScoreboardNameProvider;

macro_rules! validated_inputs {
    ($name:ident) => {
        impl<'de, V: Provider> Deserialize<'de> for $name<V> {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let value = $name::<V>::deserialize(d)?;
                value.check().map_err(D::Error::custom)?;
                Ok(value)
            }
        }

        impl<V: Provider> Serialize for $name<V> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                $name::<V>::serialize(self, s)
            }
        }
    };
}

/// A number a loot context computes, as an int or a float provider states it.
pub trait Provider:
    RegistryValue + Clone + PartialEq + fmt::Debug + Serialize + DeserializeOwned + 'static
{
    fn zero() -> Self;
}

pub fn zero_holder<V: Provider>() -> Holder<V> {
    Holder::Direct(V::zero())
}

pub fn is_zero_holder<V: Provider>(holder: &Holder<V>) -> bool {
    *holder == zero_holder()
}

/// An int computed in a loot context: a constant, or a typed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum IntExpression {
    Constant(i32),
    Typed(Box<TypedIntExpression>),
}

/// A float computed in a loot context: a constant, or a typed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum FloatExpression {
    Constant(f32),
    Typed(Box<TypedFloatExpression>),
}

impl RegistryValue for IntExpression {
    type Registry = ContextIntProvider;
}

impl RegistryValue for FloatExpression {
    type Registry = ContextFloatProvider;
}

impl Provider for IntExpression {
    fn zero() -> Self {
        IntExpression::Constant(0)
    }
}

impl Provider for FloatExpression {
    fn zero() -> Self {
        FloatExpression::Constant(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TypedIntExpression {
    #[serde(rename = "minecraft:constant", alias = "constant")]
    Constant(IntConstant),
    #[serde(rename = "minecraft:abs", alias = "abs")]
    Abs(Unary<IntExpression>),
    #[serde(rename = "minecraft:avg", alias = "avg")]
    Average(Aggregate<IntExpression>),
    #[serde(rename = "minecraft:binomial", alias = "binomial")]
    Binomial(Binomial),
    #[serde(rename = "minecraft:conditional", alias = "conditional")]
    Conditional(Conditional<IntExpression>),
    #[serde(rename = "minecraft:sub", alias = "sub")]
    Difference(Binary<IntExpression>),
    #[serde(
        rename = "minecraft:environment_attribute",
        alias = "environment_attribute"
    )]
    EnvironmentAttribute(IntAttribute),
    #[serde(rename = "minecraft:from_float", alias = "from_float")]
    FromFloat(Unary<FloatExpression>),
    #[serde(rename = "minecraft:max", alias = "max")]
    Maximum(Aggregate<IntExpression>),
    #[serde(rename = "minecraft:min", alias = "min")]
    Minimum(Aggregate<IntExpression>),
    #[serde(rename = "minecraft:floor_mod", alias = "floor_mod")]
    FloorModulus(Binary<IntExpression>),
    #[serde(rename = "minecraft:floor_div", alias = "floor_div")]
    FloorQuotient(Binary<IntExpression>),
    #[serde(rename = "minecraft:mod", alias = "mod")]
    Modulus(Binary<IntExpression>),
    #[serde(rename = "minecraft:div", alias = "div")]
    Quotient(Binary<IntExpression>),
    #[serde(rename = "minecraft:negate", alias = "negate")]
    Negate(Unary<IntExpression>),
    #[serde(rename = "minecraft:number_dispatcher", alias = "number_dispatcher")]
    NumberDispatcher(Dispatcher<IntExpression>),
    #[serde(rename = "minecraft:pow", alias = "pow")]
    Power(Power<IntExpression>),
    #[serde(rename = "minecraft:mul", alias = "mul")]
    Product(Aggregate<IntExpression>),
    #[serde(rename = "minecraft:score", alias = "score")]
    Score(Score),
    #[serde(rename = "minecraft:storage", alias = "storage")]
    Storage(Storage<IntExpression>),
    #[serde(rename = "minecraft:add", alias = "add")]
    Sum(Aggregate<IntExpression>),
    #[serde(rename = "minecraft:uniform", alias = "uniform")]
    Uniform(Range<IntExpression>),
    #[serde(rename = "minecraft:weighted_list", alias = "weighted_list")]
    WeightedList(Distribution<IntExpression>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TypedFloatExpression {
    #[serde(rename = "minecraft:constant", alias = "constant")]
    Constant(FloatConstant),
    #[serde(rename = "minecraft:abs", alias = "abs")]
    Abs(Unary<FloatExpression>),
    #[serde(rename = "minecraft:avg", alias = "avg")]
    Average(Aggregate<FloatExpression>),
    #[serde(rename = "minecraft:ceil", alias = "ceil")]
    Ceiling(Unary<FloatExpression>),
    #[serde(rename = "minecraft:conditional", alias = "conditional")]
    Conditional(Conditional<FloatExpression>),
    #[serde(rename = "minecraft:cos", alias = "cos")]
    Cosine(Unary<FloatExpression>),
    #[serde(rename = "minecraft:sub", alias = "sub")]
    Difference(Binary<FloatExpression>),
    #[serde(rename = "minecraft:enchantment_level", alias = "enchantment_level")]
    EnchantmentLevel(EnchantmentLevel),
    #[serde(
        rename = "minecraft:environment_attribute",
        alias = "environment_attribute"
    )]
    EnvironmentAttribute(FloatAttribute),
    #[serde(rename = "minecraft:floor", alias = "floor")]
    Floor(Unary<FloatExpression>),
    #[serde(rename = "minecraft:from_int", alias = "from_int")]
    FromInt(Unary<IntExpression>),
    #[serde(rename = "minecraft:length", alias = "length")]
    Length(Aggregate<FloatExpression>),
    #[serde(rename = "minecraft:max", alias = "max")]
    Maximum(Aggregate<FloatExpression>),
    #[serde(rename = "minecraft:min", alias = "min")]
    Minimum(Aggregate<FloatExpression>),
    #[serde(rename = "minecraft:mod", alias = "mod")]
    Modulus(Binary<FloatExpression>),
    #[serde(rename = "minecraft:negate", alias = "negate")]
    Negate(Unary<FloatExpression>),
    #[serde(rename = "minecraft:number_dispatcher", alias = "number_dispatcher")]
    NumberDispatcher(Dispatcher<FloatExpression>),
    #[serde(rename = "minecraft:pow", alias = "pow")]
    Power(Power<FloatExpression>),
    #[serde(rename = "minecraft:mul", alias = "mul")]
    Product(Aggregate<FloatExpression>),
    #[serde(rename = "minecraft:div", alias = "div")]
    Quotient(Binary<FloatExpression>),
    #[serde(rename = "minecraft:round", alias = "round")]
    Round(Unary<FloatExpression>),
    #[serde(rename = "minecraft:sin", alias = "sin")]
    Sine(Unary<FloatExpression>),
    #[serde(rename = "minecraft:sqrt", alias = "sqrt")]
    SquareRoot(Unary<FloatExpression>),
    #[serde(rename = "minecraft:storage", alias = "storage")]
    Storage(Storage<FloatExpression>),
    #[serde(rename = "minecraft:add", alias = "add")]
    Sum(Aggregate<FloatExpression>),
    #[serde(rename = "minecraft:truncate", alias = "truncate")]
    Truncate(Unary<FloatExpression>),
    #[serde(rename = "minecraft:uniform", alias = "uniform")]
    Uniform(Range<FloatExpression>),
    #[serde(rename = "minecraft:weighted_list", alias = "weighted_list")]
    WeightedList(Distribution<FloatExpression>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntConstant {
    #[serde(deserialize_with = "mcrs_minecraft_core::codec::int_value")]
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloatConstant {
    #[serde(deserialize_with = "mcrs_minecraft_core::codec::float_value")]
    pub value: f32,
}

impl Serialize for IntExpression {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            IntExpression::Constant(value) => s.serialize_i32(*value),
            IntExpression::Typed(typed) => typed.serialize(s),
        }
    }
}

impl Serialize for FloatExpression {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            FloatExpression::Constant(value) => s.serialize_f32(*value),
            FloatExpression::Typed(typed) => typed.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for IntExpression {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct IntVisitor {
            human_readable: bool,
        }

        impl<'de> Visitor<'de> for IntVisitor {
            type Value = IntExpression;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an int or an int provider")
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<IntExpression, E> {
                Ok(IntExpression::Constant(v as i32))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<IntExpression, E> {
                Ok(IntExpression::Constant(v as i32))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<IntExpression, E> {
                let number = Number::F64(v, self.human_readable);
                int_value(number)
                    .map(IntExpression::Constant)
                    .map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<IntExpression, A::Error> {
                match TypedIntExpression::deserialize(value::MapAccessDeserializer::new(map))? {
                    TypedIntExpression::Constant(constant) => {
                        Ok(IntExpression::Constant(constant.value))
                    }
                    typed => Ok(IntExpression::Typed(Box::new(typed))),
                }
            }
        }

        let human_readable = d.is_human_readable();
        d.deserialize_any(IntVisitor { human_readable })
    }
}

impl<'de> Deserialize<'de> for FloatExpression {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct FloatVisitor;

        impl<'de> Visitor<'de> for FloatVisitor {
            type Value = FloatExpression;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a float or a float provider")
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<FloatExpression, E> {
                Ok(FloatExpression::Constant(v as f32))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<FloatExpression, E> {
                Ok(FloatExpression::Constant(v as f32))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<FloatExpression, E> {
                Ok(FloatExpression::Constant(v as f32))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<FloatExpression, A::Error> {
                match TypedFloatExpression::deserialize(value::MapAccessDeserializer::new(map))? {
                    TypedFloatExpression::Constant(constant) => {
                        Ok(FloatExpression::Constant(constant.value))
                    }
                    typed => Ok(FloatExpression::Typed(Box::new(typed))),
                }
            }
        }

        d.deserialize_any(FloatVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Unary<V: Provider> {
    pub input: Holder<V>,
}

validated_inputs!(Aggregate);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields, bound = "")]
pub struct Aggregate<V: Provider> {
    pub inputs: HolderList<V>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Binary<V: Provider> {
    pub left: Holder<V>,
    pub right: Holder<V>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Power<V: Provider> {
    pub base: Holder<V>,
    pub exponent: Holder<V>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Range<V: Provider> {
    pub min: Holder<V>,
    pub max: Holder<V>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Conditional<V: Provider> {
    pub condition: Holder<LootCondition>,
    pub on_true: Holder<V>,
    #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
    pub on_false: Holder<V>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Dispatcher<V: Provider> {
    pub cases: Vec<DispatchCase<V>>,
    #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
    pub default: Holder<V>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct DispatchCase<V: Provider> {
    pub condition: Holder<LootCondition>,
    pub value: Holder<V>,
}

validated_inputs!(Distribution);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields, bound = "")]
pub struct Distribution<V: Provider> {
    pub distribution: Vec<Weighted<Holder<V>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
pub struct Storage<V: Provider> {
    pub storage: ResourceLocation,
    // chisle: the NBT path is kept as written and not parsed, so a malformed path passes the load; parsing it with the NBT path grammar lifts this
    pub path: String,
    #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
    pub fallback: Holder<V>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Score {
    pub target: ScoreboardNameProvider,
    pub score: String,
    #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
    pub fallback: Holder<IntExpression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binomial {
    pub n: Holder<IntExpression>,
    pub p: Holder<FloatExpression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentLevel {
    pub amount: LevelBasedValue,
}

/// An environment attribute whose value converts to an int.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntAttribute {
    pub attribute: EnvironmentAttribute,
}

/// An environment attribute whose value converts to a float.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FloatAttribute {
    pub attribute: EnvironmentAttribute,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttributeRepr {
    attribute: EnvironmentAttribute,
}

fn attribute_type(id: EnvironmentAttribute) -> Option<AttributeType> {
    attribute(id.as_static_str()).map(|spec| spec.ty)
}

impl<'de> Deserialize<'de> for IntAttribute {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let AttributeRepr { attribute } = AttributeRepr::deserialize(d)?;
        match attribute_type(attribute) {
            Some(AttributeType::Integer) => Ok(IntAttribute { attribute }),
            _ => Err(D::Error::custom(format_args!(
                "{} cannot be converted to an integer",
                attribute.as_static_str()
            ))),
        }
    }
}

impl<'de> Deserialize<'de> for FloatAttribute {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let AttributeRepr { attribute } = AttributeRepr::deserialize(d)?;
        match attribute_type(attribute) {
            Some(AttributeType::Integer | AttributeType::Float | AttributeType::AngleDegrees) => {
                Ok(FloatAttribute { attribute })
            }
            _ => Err(D::Error::custom(format_args!(
                "{} cannot be converted to a float",
                attribute.as_static_str()
            ))),
        }
    }
}

/// An int that must lie in a range, both ends computed in the loot context.
#[derive(Debug, Clone, PartialEq)]
pub enum IntRangePredicate {
    Point(Holder<IntExpression>),
    Line {
        min: Option<Holder<IntExpression>>,
        max: Option<Holder<IntExpression>>,
    },
}

/// A float that must lie in a range, both ends computed in the loot context.
#[derive(Debug, Clone, PartialEq)]
pub enum FloatRangePredicate {
    Point(Holder<FloatExpression>),
    Line {
        min: Option<Holder<FloatExpression>>,
        max: Option<Holder<FloatExpression>>,
    },
}

macro_rules! range_predicate {
    ($name:ident, $provider:ty) => {
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self {
                    $name::Point(value) => value.serialize(s),
                    $name::Line { min, max } => RangeLine {
                        min: min.clone(),
                        max: max.clone(),
                    }
                    .serialize(s),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct PointOrLine;

                impl<'de> Visitor<'de> for PointOrLine {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str("a value or a {min, max} range")
                    }

                    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<$name, E> {
                        Holder::deserialize(value::I64Deserializer::new(v)).map($name::Point)
                    }

                    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<$name, E> {
                        Holder::deserialize(value::U64Deserializer::new(v)).map($name::Point)
                    }

                    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<$name, E> {
                        Holder::deserialize(value::F64Deserializer::new(v)).map($name::Point)
                    }

                    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<$name, E> {
                        Holder::deserialize(value::StrDeserializer::new(v)).map($name::Point)
                    }

                    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<$name, A::Error> {
                        // A provider written inline is a map with a type; a range never has one.
                        let entries = MapEntries::read(map)?;
                        if entries.has("type") {
                            entries.into_holder().map($name::Point)
                        } else {
                            let line: RangeLine<$provider> = entries.into_value()?;
                            Ok($name::Line {
                                min: line.min,
                                max: line.max,
                            })
                        }
                    }
                }

                d.deserialize_any(PointOrLine)
            }
        }
    };
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct RangeLine<V: Provider> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min: Option<Holder<V>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max: Option<Holder<V>>,
}

range_predicate!(IntRangePredicate, IntExpression);
range_predicate!(FloatRangePredicate, FloatExpression);

impl<V: Provider> Aggregate<V> {
    fn check(&self) -> Result<(), String> {
        match &self.inputs {
            HolderList::List(inputs) if inputs.is_empty() => {
                Err("inputs: must have at least 1 element".into())
            }
            _ => Ok(()),
        }
    }
}

impl<V: Provider> Distribution<V> {
    fn check(&self) -> Result<(), String> {
        if self.distribution.is_empty() {
            Err("distribution: must not be empty".into())
        } else {
            Ok(())
        }
    }
}

impl IntExpression {
    /// The type of the expression at the root, the part an item can name.
    pub fn kind(&self) -> ContextIntProviderType {
        match self {
            IntExpression::Constant(_) => ContextIntProviderType::Constant,
            IntExpression::Typed(typed) => match **typed {
                TypedIntExpression::Constant(_) => ContextIntProviderType::Constant,
                TypedIntExpression::Abs(_) => ContextIntProviderType::Abs,
                TypedIntExpression::Average(_) => ContextIntProviderType::Avg,
                TypedIntExpression::Binomial(_) => ContextIntProviderType::Binomial,
                TypedIntExpression::Conditional(_) => ContextIntProviderType::Conditional,
                TypedIntExpression::Difference(_) => ContextIntProviderType::Sub,
                TypedIntExpression::EnvironmentAttribute(_) => {
                    ContextIntProviderType::EnvironmentAttribute
                }
                TypedIntExpression::FromFloat(_) => ContextIntProviderType::FromFloat,
                TypedIntExpression::Maximum(_) => ContextIntProviderType::Max,
                TypedIntExpression::Minimum(_) => ContextIntProviderType::Min,
                TypedIntExpression::FloorModulus(_) => ContextIntProviderType::FloorMod,
                TypedIntExpression::FloorQuotient(_) => ContextIntProviderType::FloorDiv,
                TypedIntExpression::Modulus(_) => ContextIntProviderType::Mod,
                TypedIntExpression::Quotient(_) => ContextIntProviderType::Div,
                TypedIntExpression::Negate(_) => ContextIntProviderType::Negate,
                TypedIntExpression::NumberDispatcher(_) => ContextIntProviderType::NumberDispatcher,
                TypedIntExpression::Power(_) => ContextIntProviderType::Pow,
                TypedIntExpression::Product(_) => ContextIntProviderType::Mul,
                TypedIntExpression::Score(_) => ContextIntProviderType::Score,
                TypedIntExpression::Storage(_) => ContextIntProviderType::Storage,
                TypedIntExpression::Sum(_) => ContextIntProviderType::Add,
                TypedIntExpression::Uniform(_) => ContextIntProviderType::Uniform,
                TypedIntExpression::WeightedList(_) => ContextIntProviderType::WeightedList,
            },
        }
    }

    pub fn split(expression: &Self) -> (ContextIntProvider, Self) {
        (
            ContextIntProvider {
                kind: expression.kind(),
            },
            expression.clone(),
        )
    }

    pub fn join((_, expression): (&ContextIntProvider, &Self)) -> Self {
        expression.clone()
    }
}

impl FloatExpression {
    /// The type of the expression at the root, the part an item can name.
    pub fn kind(&self) -> ContextFloatProviderType {
        match self {
            FloatExpression::Constant(_) => ContextFloatProviderType::Constant,
            FloatExpression::Typed(typed) => match **typed {
                TypedFloatExpression::Constant(_) => ContextFloatProviderType::Constant,
                TypedFloatExpression::Abs(_) => ContextFloatProviderType::Abs,
                TypedFloatExpression::Average(_) => ContextFloatProviderType::Avg,
                TypedFloatExpression::Ceiling(_) => ContextFloatProviderType::Ceil,
                TypedFloatExpression::Conditional(_) => ContextFloatProviderType::Conditional,
                TypedFloatExpression::Cosine(_) => ContextFloatProviderType::Cos,
                TypedFloatExpression::Difference(_) => ContextFloatProviderType::Sub,
                TypedFloatExpression::EnchantmentLevel(_) => {
                    ContextFloatProviderType::EnchantmentLevel
                }
                TypedFloatExpression::EnvironmentAttribute(_) => {
                    ContextFloatProviderType::EnvironmentAttribute
                }
                TypedFloatExpression::Floor(_) => ContextFloatProviderType::Floor,
                TypedFloatExpression::FromInt(_) => ContextFloatProviderType::FromInt,
                TypedFloatExpression::Length(_) => ContextFloatProviderType::Length,
                TypedFloatExpression::Maximum(_) => ContextFloatProviderType::Max,
                TypedFloatExpression::Minimum(_) => ContextFloatProviderType::Min,
                TypedFloatExpression::Modulus(_) => ContextFloatProviderType::Mod,
                TypedFloatExpression::Negate(_) => ContextFloatProviderType::Negate,
                TypedFloatExpression::NumberDispatcher(_) => {
                    ContextFloatProviderType::NumberDispatcher
                }
                TypedFloatExpression::Power(_) => ContextFloatProviderType::Pow,
                TypedFloatExpression::Product(_) => ContextFloatProviderType::Mul,
                TypedFloatExpression::Quotient(_) => ContextFloatProviderType::Div,
                TypedFloatExpression::Round(_) => ContextFloatProviderType::Round,
                TypedFloatExpression::Sine(_) => ContextFloatProviderType::Sin,
                TypedFloatExpression::SquareRoot(_) => ContextFloatProviderType::Sqrt,
                TypedFloatExpression::Storage(_) => ContextFloatProviderType::Storage,
                TypedFloatExpression::Sum(_) => ContextFloatProviderType::Add,
                TypedFloatExpression::Truncate(_) => ContextFloatProviderType::Truncate,
                TypedFloatExpression::Uniform(_) => ContextFloatProviderType::Uniform,
                TypedFloatExpression::WeightedList(_) => ContextFloatProviderType::WeightedList,
            },
        }
    }

    pub fn split(expression: &Self) -> (ContextFloatProvider, Self) {
        (
            ContextFloatProvider {
                kind: expression.kind(),
            },
            expression.clone(),
        )
    }

    pub fn join((_, expression): (&ContextFloatProvider, &Self)) -> Self {
        expression.clone()
    }
}
