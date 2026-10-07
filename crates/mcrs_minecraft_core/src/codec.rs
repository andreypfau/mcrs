use std::fmt;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, nbt_int_array};
use serde::de::{DeserializeOwned, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// `Codec.INT` / `Codec.LONG`: any number's `intValue()` / `longValue()`. An
/// integer keeps its low bits; a fraction is dropped, and a value beyond the
/// range keeps its low bits from JSON (`BigDecimal.intValue`) but saturates
/// from NBT (`Double.intValue`). Vanilla wraps the exact decimal text, while
/// serde hands over the parsed double, so a text no double holds exactly
/// (`1e40`) wraps to a different value.
macro_rules! java_integer_value {
    ($name:ident, $ty:ty) => {
        pub fn $name<'de, D: Deserializer<'de>>(d: D) -> Result<$ty, D::Error> {
            struct Value {
                wrap_floats: bool,
            }

            impl Visitor<'_> for Value {
                type Value = $ty;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a number")
                }

                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<$ty, E> {
                    Ok(v as $ty)
                }

                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<$ty, E> {
                    Ok(v as $ty)
                }

                fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<$ty, E> {
                    if !self.wrap_floats {
                        return Ok(v as $ty);
                    }
                    let truncated = v.trunc();
                    Ok(if truncated.abs() >= 2f64.powi(127) {
                        0
                    } else {
                        truncated as i128 as $ty
                    })
                }
            }

            let wrap_floats = d.is_human_readable();
            d.deserialize_any(Value { wrap_floats })
        }
    };
}

java_integer_value!(int_value, i32);
java_integer_value!(long_value, i64);

/// `Codec.FLOAT`: any number's `floatValue()`.
pub fn float_value<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    f32::deserialize(d)
}

/// A `Codec.intRange(MIN, MAX)` payload, with the value `optionalFieldOf`
/// falls back to. Stated once here rather than as a validator per field,
/// because the tree registries carry a dozen distinct bounds across sixty-odd
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bounded<const MIN: i32, const MAX: i32, const DEFAULT: i32 = 0>(pub i32);

impl<const MIN: i32, const MAX: i32, const DEFAULT: i32> Default for Bounded<MIN, MAX, DEFAULT> {
    fn default() -> Self {
        Bounded(DEFAULT)
    }
}

impl<const MIN: i32, const MAX: i32, const DEFAULT: i32> Bounded<MIN, MAX, DEFAULT> {
    pub fn new(value: i32) -> Result<Self, String> {
        if (MIN..=MAX).contains(&value) {
            Ok(Bounded(value))
        } else {
            Err(Self::out_of_range(value))
        }
    }

    fn out_of_range(value: i32) -> String {
        match (MIN, MAX) {
            (0, i32::MAX) => format!("Value must be non-negative: {value}"),
            (1, i32::MAX) => format!("Value must be positive: {value}"),
            _ => format!("Value must be within range [{MIN};{MAX}]: {value}"),
        }
    }
}

pub fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

pub fn default_true() -> bool {
    true
}

pub fn is_true(value: &bool) -> bool {
    *value
}

/// One element writes bare, any other count writes a list; a bare element
/// reads as a one-element list.
#[derive(Clone, Debug, PartialEq)]
pub struct CompactList<T>(pub Vec<T>);

impl<T> Default for CompactList<T> {
    fn default() -> Self {
        CompactList(Vec::new())
    }
}

impl<T> CompactList<T> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Serialize> Serialize for CompactList<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        compact_list::serialize(&self.0, s)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for CompactList<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ListVisitor<T>(std::marker::PhantomData<fn() -> T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for ListVisitor<T> {
            type Value = CompactList<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list or a single element")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(CompactList)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(value::MapAccessDeserializer::new(map)).map(|v| CompactList(vec![v]))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                T::deserialize(value::StrDeserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                T::deserialize(value::BoolDeserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                T::deserialize(value::I64Deserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                T::deserialize(value::U64Deserializer::new(v)).map(|v| CompactList(vec![v]))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                T::deserialize(value::F64Deserializer::new(v)).map(|v| CompactList(vec![v]))
            }
        }

        d.deserialize_any(ListVisitor(std::marker::PhantomData))
    }
}

/// [`CompactList`] for a field that holds a plain `Vec`.
pub mod compact_list {
    use super::CompactList;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: Serialize, S: Serializer>(items: &[T], s: S) -> Result<S::Ok, S::Error> {
        match items {
            [only] => only.serialize(s),
            items => items.serialize(s),
        }
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<T>, D::Error> {
        CompactList::deserialize(d).map(|list| list.0)
    }
}

impl<'de, const MIN: i32, const MAX: i32, const DEFAULT: i32> Deserialize<'de>
    for Bounded<MIN, MAX, DEFAULT>
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = int_value(deserializer)?;
        if !(MIN..=MAX).contains(&value) {
            return Err(D::Error::custom(Self::out_of_range(value)));
        }
        Ok(Bounded(value))
    }
}

impl<const MIN: i32, const MAX: i32, const DEFAULT: i32> Serialize for Bounded<MIN, MAX, DEFAULT> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !(MIN..=MAX).contains(&self.0) {
            return Err(serde::ser::Error::custom(Self::out_of_range(self.0)));
        }
        self.0.serialize(serializer)
    }
}

pub type NonNegativeInt = Bounded<0, { i32::MAX }>;
pub type PositiveInt = Bounded<1, { i32::MAX }>;

/// `ExtraCodecs.POSITIVE_FLOAT`: the low bound is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32")]
pub struct PositiveFloat(pub f32);

impl TryFrom<f32> for PositiveFloat {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        if !(value > 0.0 && value <= f32::MAX) {
            return Err(format!("Value must be positive: {value}"));
        }
        Ok(PositiveFloat(value))
    }
}

pub fn non_empty<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let values = Vec::<T>::deserialize(deserializer)?;
    if values.is_empty() {
        return Err(D::Error::custom("List must have contents"));
    }
    Ok(values)
}

pub fn sized_list<'de, const MIN: usize, const MAX: usize, D, T>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let values = Vec::<T>::deserialize(deserializer)?;
    let size = values.len();
    if size < MIN {
        return Err(D::Error::custom(format_args!(
            "List is too short: {size}, expected range [{MIN}-{MAX}]"
        )));
    }
    if size > MAX {
        return Err(D::Error::custom(format_args!(
            "List is too long: {size}, expected range [{MIN}-{MAX}]"
        )));
    }
    Ok(values)
}

/// A codec bound the field list alone does not express. The shape is derived as
/// usual and `validated!` hangs the check on the way in, so the fields are
/// spelled once rather than once more in a shadow struct that has to be kept in
/// step by hand.
pub trait Validate: Sized {
    fn validate(&self) -> Result<(), String>;
}

/// Turns the inherent codec `#[serde(remote = "Self")]` generates back into the
/// trait impls, checking [`Validate`] on the way in.
#[macro_export]
macro_rules! validated {
    ($($name:ident),* $(,)?) => {$(
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let value = $name::deserialize(deserializer)?;
                value.validate().map_err(serde::de::Error::custom)?;
                Ok(value)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                $name::serialize(self, serializer)
            }
        }
    )*};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_value_reads_any_number_as_java_does() {
        let read = |json: &str| serde_json::from_str::<NonNegativeInt>(json);
        assert_eq!(read("1.5").unwrap().0, 1);
        assert_eq!(read("2.9").unwrap().0, 2);
        assert_eq!(read("1e10").unwrap().0, 1410065408);
        assert_eq!(read("4294967297").unwrap().0, 1);
        assert_eq!(
            read("3000000000.0").unwrap_err().to_string(),
            "Value must be non-negative: -1294967296"
        );
        assert_eq!(read("1e300").unwrap().0, 0);
    }

    #[test]
    fn a_positive_float_is_a_float_up_to_the_largest_finite_one() {
        let read = |json: &str| serde_json::from_str::<PositiveFloat>(json).map(|value| value.0);
        assert_eq!(read("0.5").unwrap(), 0.5);
        assert_eq!(read("3.4028235e38").unwrap(), f32::MAX);
        for out_of_range in ["0", "-0.0", "-1", "1e39"] {
            assert!(read(out_of_range).is_err(), "{out_of_range}");
        }
        assert_eq!(
            mcrs_minecraft_nbt::to_nbt_tag(&PositiveFloat(0.5)).unwrap(),
            NbtTag::Float(0.5)
        );
    }
}

/// A number a visitor already holds, replayed through the int and float
/// readers; the flag is the source's `is_human_readable`.
pub enum Number {
    I64(i64, bool),
    U64(u64, bool),
    F64(f64, bool),
}

impl<'de> Deserializer<'de> for Number {
    type Error = value::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self {
            Number::I64(v, _) => visitor.visit_i64(v),
            Number::U64(v, _) => visitor.visit_u64(v),
            Number::F64(v, _) => visitor.visit_f64(v),
        }
    }

    fn is_human_readable(&self) -> bool {
        match *self {
            Number::I64(_, h) | Number::U64(_, h) | Number::F64(_, h) => h,
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }
}

/// Any number, truncated to an int with the low byte kept.
pub fn unsigned_byte<'de, D: Deserializer<'de>>(d: D) -> Result<i8, D::Error> {
    int_value(d).map(|v| v as i8)
}

/// A present-but-malformed value reads as absent, and so does a JSON `null`.
/// The value is buffered first so a failed parse never leaves a streaming
/// input half-consumed.
pub fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    d: D,
) -> Result<T, D::Error> {
    let tag = <Option<NbtTag> as Deserialize>::deserialize(d)?;
    Ok(tag.and_then(|tag| from_tag(tag).ok()).unwrap_or_default())
}

/// Only a number reads as a value;
/// anything else present (a JSON boolean or null included) is consumed and
/// reads as absent.
pub fn lenient_float<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    struct LenientFloat;

    impl<'de> Visitor<'de> for LenientFloat {
        type Value = Option<f32>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("any value")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }

        fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_bytes<E: serde::de::Error>(self, _: &[u8]) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            Ok(None)
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok(None)
        }
    }

    d.deserialize_any(LenientFloat)
}

/// A compound, or on read an SNBT string that parses to one.
pub fn compound_or_snbt<'de, D: Deserializer<'de>>(d: D) -> Result<NbtCompound, D::Error> {
    struct CompoundVisitor;

    impl<'de> Visitor<'de> for CompoundVisitor {
        type Value = NbtCompound;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a compound or its SNBT text")
        }

        fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<NbtCompound, E> {
            mcrs_minecraft_nbt::snbt::parse_compound(text).map_err(E::custom)
        }

        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<NbtCompound, A::Error> {
            NbtCompound::deserialize(value::MapAccessDeserializer::new(map))
        }
    }

    d.deserialize_any(CompoundVisitor)
}

/// Each channel is floored to eight bits and masked, so an out-of-range
/// channel never bleeds into its neighbour.
pub fn argb_from_floats(a: f32, r: f32, g: f32, b: f32) -> i32 {
    let channel = |v: f32| (v * 255.0).floor() as i32 & 0xFF;
    (channel(a) << 24) | (channel(r) << 16) | (channel(g) << 8) | channel(b)
}

/// An int, or on read `N` float channels in 0..=1.
macro_rules! color_int {
    ($(#[$meta:meta])* $name:ident, $channels:literal, $from:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub i32);

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_i32(self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct ColorVisitor(bool);

                impl<'de> Visitor<'de> for ColorVisitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, "an int or {} color channels", $channels)
                    }

                    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<$name, E> {
                        int_value(Number::I64(v, self.0)).map($name).map_err(E::custom)
                    }

                    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<$name, E> {
                        int_value(Number::U64(v, self.0)).map($name).map_err(E::custom)
                    }

                    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<$name, E> {
                        int_value(Number::F64(v, self.0)).map($name).map_err(E::custom)
                    }

                    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<$name, A::Error> {
                        let channels: Vec<f32> =
                            Vec::deserialize(value::SeqAccessDeserializer::new(seq))?;
                        let channels: [f32; $channels] = channels.try_into().map_err(|c: Vec<f32>| {
                            A::Error::custom(format_args!(
                                "expected {} color channels, got {}",
                                $channels,
                                c.len()
                            ))
                        })?;
                        Ok($name($from(channels)))
                    }
                }

                let human_readable = d.is_human_readable();
                d.deserialize_any(ColorVisitor(human_readable))
            }
        }
    };
}

color_int!(
    /// An int, or on read `[r, g, b]`.
    RgbInt,
    3,
    |[r, g, b]: [f32; 3]| argb_from_floats(1.0, r, g, b)
);

color_int!(
    /// An int, or on read `[r, g, b, a]`.
    ArgbInt,
    4,
    |[r, g, b, a]: [f32; 4]| argb_from_floats(a, r, g, b)
);

/// `ExtraCodecs.STRING_RGB_COLOR`: written as `#rrggbb`; read from that, from
/// a packed int, or from `[r, g, b]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct HexRgb(pub u32);

impl HexRgb {
    pub const fn of(packed: i32) -> Self {
        HexRgb(packed as u32 & 0x00FF_FFFF)
    }
}

impl Serialize for HexRgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("#{:06x}", self.0))
    }
}

impl<'de> Deserialize<'de> for HexRgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct HexVisitor;

        impl<'de> Visitor<'de> for HexVisitor {
            type Value = HexRgb;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("`#rrggbb`, a packed int or three float channels")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<HexRgb, E> {
                v.strip_prefix('#')
                    .filter(|hex| hex.len() == 6)
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .map(HexRgb)
                    .ok_or_else(|| E::custom(format!("{v} is not a `#rrggbb` colour")))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<HexRgb, E> {
                Ok(HexRgb::of(v as i32))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<HexRgb, E> {
                Ok(HexRgb::of(v as i32))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<HexRgb, A::Error> {
                let [r, g, b] = <[f32; 3]>::deserialize(value::SeqAccessDeserializer::new(seq))?;
                Ok(HexRgb::of(argb_from_floats(1.0, r, g, b)))
            }
        }

        deserializer.deserialize_any(HexVisitor)
    }
}

/// Bounded in UTF-16 code units.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
#[serde(transparent)]
pub struct BoundedString<const MAX_CHARS: usize>(pub String);

impl<const MAX_CHARS: usize> BoundedString<MAX_CHARS> {
    pub fn new(text: impl Into<String>) -> Result<Self, String> {
        let text = text.into();
        let chars = text.encode_utf16().count();
        if chars > MAX_CHARS {
            return Err(format!(
                "String \"{text}\" is too long: {chars}, expected range [0-{MAX_CHARS}]"
            ));
        }
        Ok(BoundedString(text))
    }
}

impl<'de, const MAX_CHARS: usize> Deserialize<'de> for BoundedString<MAX_CHARS> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        BoundedString::new(String::deserialize(d)?).map_err(D::Error::custom)
    }
}

/// A fixed-length int array: a plain array in JSON, a `TAG_Int_Array` in NBT
/// and in the hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntArray<const N: usize>(pub [i32; N]);

impl<const N: usize> Serialize for IntArray<N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            self.0[..].serialize(s)
        } else {
            nbt_int_array(&self.0[..], s)
        }
    }
}

impl<'de, const N: usize> Deserialize<'de> for IntArray<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Element(i32);

        impl<'de> Deserialize<'de> for Element {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                int_value(d).map(Element)
            }
        }

        let values = Vec::<Element>::deserialize(d)?;
        let len = values.len();
        values
            .into_iter()
            .map(|Element(v)| v)
            .collect::<Vec<i32>>()
            .try_into()
            .map(IntArray)
            .map_err(|_| D::Error::custom(format_args!("expected {N} ints, got {len}")))
    }
}

/// A boolean that is not optional: see [`optional_flag`].
pub fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    optional_flag(deserializer)?
        .ok_or_else(|| D::Error::custom("expected a boolean or the byte NBT stores one as"))
}

/// NBT stores a boolean as a byte, and serde's buffered `untagged` and
/// `flatten` paths lose the deserializer's own coercion, so accept both.
pub fn optional_flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    struct FlagVisitor;

    impl<'de> Visitor<'de> for FlagVisitor {
        type Value = Option<bool>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a boolean or the byte NBT stores one as")
        }

        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v != 0))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v != 0))
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_any(FlagVisitor)
        }
    }

    deserializer.deserialize_any(FlagVisitor)
}
