use std::fmt::Display;

use crc32c::{crc32c, crc32c_append};
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::ser::{self, Error as _, Impossible, Serialize};

const EMPTY: u8 = 1;
const MAP_START: u8 = 2;
const MAP_END: u8 = 3;
const LIST_START: u8 = 4;
const LIST_END: u8 = 5;
const BYTE: u8 = 6;
const SHORT: u8 = 7;
const INT: u8 = 8;
const LONG: u8 = 9;
const FLOAT: u8 = 10;
const DOUBLE: u8 = 11;
const STRING: u8 = 12;
const BOOLEAN: u8 = 13;
const BYTE_ARRAY_START: u8 = 14;
const BYTE_ARRAY_END: u8 = 15;
const INT_ARRAY_START: u8 = 16;
const INT_ARRAY_END: u8 = 17;
const LONG_ARRAY_START: u8 = 18;
const LONG_ARRAY_END: u8 = 19;

const NBT_ARRAY_TAG: &str = "__nbt_array";

#[derive(Debug, thiserror::Error)]
pub enum HashError {
    #[error("{0}")]
    Custom(String),
    #[error("{0} has no persistent form")]
    Unsupported(&'static str),
}

impl ser::Error for HashError {
    fn custom<T: Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}

type Hashed = Option<u32>;
type Result<T = Hashed> = std::result::Result<T, HashError>;

pub fn hash<T: Serialize + ?Sized>(value: &T) -> Result<i32> {
    Ok(value.serialize(HashSerializer)?.unwrap_or_else(empty) as i32)
}

fn empty() -> u32 {
    crc32c(&[EMPTY])
}

fn scalar(tag: u8, payload: &[u8]) -> Result {
    Ok(Some(crc32c_append(crc32c(&[tag]), payload)))
}

pub struct HashSerializer;

impl ser::Serializer for HashSerializer {
    type Ok = Hashed;
    type Error = HashError;
    type SerializeSeq = ListHasher;
    type SerializeTuple = ListHasher;
    type SerializeTupleStruct = Impossible<Hashed, HashError>;
    type SerializeTupleVariant = Impossible<Hashed, HashError>;
    type SerializeMap = MapHasher;
    type SerializeStruct = MapHasher;
    type SerializeStructVariant = Impossible<Hashed, HashError>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, v: bool) -> Result {
        scalar(BOOLEAN, &[v as u8])
    }

    fn serialize_i8(self, v: i8) -> Result {
        scalar(BYTE, &v.to_le_bytes())
    }

    fn serialize_i16(self, v: i16) -> Result {
        scalar(SHORT, &v.to_le_bytes())
    }

    fn serialize_i32(self, v: i32) -> Result {
        scalar(INT, &v.to_le_bytes())
    }

    fn serialize_i64(self, v: i64) -> Result {
        scalar(LONG, &v.to_le_bytes())
    }

    fn serialize_u8(self, v: u8) -> Result {
        scalar(BYTE, &[v])
    }

    fn serialize_u16(self, _: u16) -> Result {
        Err(HashError::Unsupported("u16"))
    }

    fn serialize_u32(self, _: u32) -> Result {
        Err(HashError::Unsupported("u32"))
    }

    fn serialize_u64(self, _: u64) -> Result {
        Err(HashError::Unsupported("u64"))
    }

    fn serialize_f32(self, v: f32) -> Result {
        scalar(FLOAT, &v.to_le_bytes())
    }

    fn serialize_f64(self, v: f64) -> Result {
        scalar(DOUBLE, &v.to_le_bytes())
    }

    fn serialize_char(self, _: char) -> Result {
        Err(HashError::Unsupported("char"))
    }

    fn serialize_str(self, v: &str) -> Result {
        let units: Vec<u16> = v.encode_utf16().collect();
        let mut bytes = Vec::with_capacity(5 + units.len() * 2);
        bytes.push(STRING);
        bytes.extend_from_slice(&(units.len() as i32).to_le_bytes());
        bytes.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
        Ok(Some(crc32c(&bytes)))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result {
        let mut list = ListHasher::default();
        for byte in v {
            list.push(scalar(BYTE, &[*byte])?);
        }
        ser::SerializeSeq::end(list)
    }

    fn serialize_none(self) -> Result {
        Ok(None)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result {
        Ok(None)
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result {
        Err(HashError::Unsupported("unit struct"))
    }

    fn serialize_unit_variant(self, _: &'static str, _: u32, variant: &'static str) -> Result {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, _: &T) -> Result {
        Err(HashError::Unsupported("newtype struct"))
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result {
        if name != NBT_ARRAY_TAG {
            return Err(HashError::Unsupported("newtype variant"));
        }
        let tag = mcrs_minecraft_nbt::tag_serializer::TagSerializer
            .serialize_newtype_variant(name, 0, variant, value)
            .map_err(HashError::custom)?;
        let (start, end, payload) = match tag {
            NbtTag::ByteArray(bytes) => (BYTE_ARRAY_START, BYTE_ARRAY_END, bytes.to_vec()),
            NbtTag::IntArray(ints) => (
                INT_ARRAY_START,
                INT_ARRAY_END,
                ints.iter().flat_map(|v| v.to_le_bytes()).collect(),
            ),
            NbtTag::LongArray(longs) => (
                LONG_ARRAY_START,
                LONG_ARRAY_END,
                longs.iter().flat_map(|v| v.to_le_bytes()).collect(),
            ),
            _ => return Err(HashError::Unsupported("array marker")),
        };
        let mut bytes = Vec::with_capacity(payload.len() + 2);
        bytes.push(start);
        bytes.extend_from_slice(&payload);
        bytes.push(end);
        Ok(Some(crc32c(&bytes)))
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<ListHasher> {
        Ok(ListHasher::default())
    }

    fn serialize_tuple(self, _: usize) -> Result<ListHasher> {
        Ok(ListHasher::default())
    }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(HashError::Unsupported("tuple struct"))
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(HashError::Unsupported("tuple variant"))
    }

    fn serialize_map(self, _: Option<usize>) -> Result<MapHasher> {
        Ok(MapHasher::default())
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<MapHasher> {
        Ok(MapHasher::default())
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(HashError::Unsupported("struct variant"))
    }
}

pub struct ListHasher(Vec<u8>);

impl Default for ListHasher {
    fn default() -> Self {
        Self(vec![LIST_START])
    }
}

impl ListHasher {
    fn push(&mut self, element: Hashed) {
        self.0
            .extend_from_slice(&element.unwrap_or_else(empty).to_le_bytes());
    }
}

impl ser::SerializeSeq for ListHasher {
    type Ok = Hashed;
    type Error = HashError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.push(value.serialize(HashSerializer)?);
        Ok(())
    }

    fn end(mut self) -> Result {
        self.0.push(LIST_END);
        Ok(Some(crc32c(&self.0)))
    }
}

impl ser::SerializeTuple for ListHasher {
    type Ok = Hashed;
    type Error = HashError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result {
        ser::SerializeSeq::end(self)
    }
}

#[derive(Default)]
pub struct MapHasher {
    entries: Vec<(u32, u32)>,
    key: Option<u32>,
}

impl ser::SerializeMap for MapHasher {
    type Ok = Hashed;
    type Error = HashError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        self.key = Some(
            key.serialize(HashSerializer)?
                .ok_or(HashError::Unsupported("absent map key"))?,
        );
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        let key = self
            .key
            .take()
            .ok_or(HashError::Unsupported("map value before its key"))?;
        if let Some(value) = value.serialize(HashSerializer)? {
            self.entries.push((key, value));
        }
        Ok(())
    }

    fn end(mut self) -> Result {
        self.entries.sort_unstable();
        let mut bytes = Vec::with_capacity(2 + self.entries.len() * 8);
        bytes.push(MAP_START);
        for (key, value) in self.entries {
            bytes.extend_from_slice(&key.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(MAP_END);
        Ok(Some(crc32c(&bytes)))
    }
}

impl ser::SerializeStruct for MapHasher {
    type Ok = Hashed;
    type Error = HashError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(self) -> Result {
        ser::SerializeMap::end(self)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::{Serialize, Serializer};

    use super::{HashSerializer, hash};

    // Golden values printed by the 26.3-snapshot-10 client; 26.3 kept `HashOps` unchanged.
    const EMPTY: i32 = -1609117614;
    const EMPTY_MAP: i32 = -982207288;
    const EMPTY_LIST: i32 = -1978007022;
    const A_BYTE_1: i32 = -1990988826;
    const A_INT_1: i32 = -108872706;
    const A_SHORT_NEG2: i32 = 2053373670;
    const STR_EMPTY: i32 = 1615905556;
    const STR_HELLO: i32 = 773640809;
    const STR_ASTRAL: i32 = 2126615229;
    const LIST_INTS: i32 = 2000803920;
    const NESTED: i32 = -1698105082;
    const INT_ARRAY: i32 = 709107336;
    const LONG_ARRAY: i32 = -309389825;
    const BYTE_ARRAY: i32 = 1491979440;
    const FLOAT: i32 = -1709970540;
    const DOUBLE: i32 = -17727433;
    const LONG: i32 = -556570646;
    const BOOL_TRUE: i32 = -1019818302;
    const BOOL_FALSE: i32 = 828198337;
    const BYTE_TRUE: i32 = 1791337955;
    const LIST_OF_BOOLS: i32 = 1072798967;
    const SIXTEEN_KEYS: i32 = 1387017585;
    const MIXED_SIGN_KEYS: i32 = 1656600283;
    const KEY_A: i32 = 754812039;
    const KEY_H: i32 = -1572085944;
    const KEY_P: i32 = 10153329;

    fn map<V: Serialize + Clone>(entries: &[(&str, V)]) -> BTreeMap<String, V> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn scalars_match_vanilla() {
        assert_eq!(hash(&1.5f32).unwrap(), FLOAT);
        assert_eq!(hash(&-2.25f64).unwrap(), DOUBLE);
        assert_eq!(hash(&1234567890123i64).unwrap(), LONG);
        assert_eq!(hash(&true).unwrap(), BOOL_TRUE);
        assert_eq!(hash(&false).unwrap(), BOOL_FALSE);
        assert_eq!(hash(&1i8).unwrap(), BYTE_TRUE);
        assert_eq!(hash(&"").unwrap(), STR_EMPTY);
        assert_eq!(hash(&"hello").unwrap(), STR_HELLO);
        assert_eq!(hash(&"a\u{1F600}b").unwrap(), STR_ASTRAL);
    }

    #[test]
    fn empties_match_vanilla() {
        assert_eq!(hash(&()).unwrap(), EMPTY);
        assert_eq!(hash(&None::<i32>).unwrap(), EMPTY);
        assert_eq!(hash(&BTreeMap::<String, i32>::new()).unwrap(), EMPTY_MAP);
        assert_eq!(hash(&Vec::<i32>::new()).unwrap(), EMPTY_LIST);
    }

    #[test]
    fn single_entry_maps_match_vanilla() {
        assert_eq!(hash(&map(&[("a", 1i8)])).unwrap(), A_BYTE_1);
        assert_eq!(hash(&map(&[("a", 1i32)])).unwrap(), A_INT_1);
        assert_eq!(hash(&map(&[("a", -2i16)])).unwrap(), A_SHORT_NEG2);
    }

    #[test]
    fn lists_match_vanilla() {
        assert_eq!(hash(&vec![1i32, -2, 300]).unwrap(), LIST_INTS);
        assert_eq!(hash(&(1i32, -2i32, 300i32)).unwrap(), LIST_INTS);
        assert_eq!(hash(&[1i32, -2, 300]).unwrap(), LIST_INTS);
        assert_eq!(hash(&[true, false]).unwrap(), LIST_OF_BOOLS);
    }

    #[test]
    fn arrays_match_vanilla() {
        let ints = vec![7, -8, 9];
        let longs = [1i64 << 40, -1];
        let bytes = vec![1i8, 2, -3];
        assert_eq!(
            mcrs_minecraft_nbt::nbt_int_array(&ints, HashSerializer)
                .unwrap()
                .unwrap() as i32,
            INT_ARRAY
        );
        assert_eq!(
            mcrs_minecraft_nbt::nbt_long_array(longs, HashSerializer)
                .unwrap()
                .unwrap() as i32,
            LONG_ARRAY
        );
        assert_eq!(
            mcrs_minecraft_nbt::nbt_byte_array(&bytes, HashSerializer)
                .unwrap()
                .unwrap() as i32,
            BYTE_ARRAY
        );
        assert!(mcrs_minecraft_nbt::nbt_int_array([1i64], HashSerializer).is_err());
        assert!(mcrs_minecraft_nbt::nbt_int_array(["x"], HashSerializer).is_err());
    }

    #[test]
    fn nested_struct_matches_vanilla() {
        #[derive(Serialize)]
        struct Inner {
            x: &'static str,
            n: i64,
        }
        #[derive(Serialize)]
        struct Nested {
            inner: Inner,
            flag: i8,
            #[serde(serialize_with = "mcrs_minecraft_nbt::nbt_int_array")]
            pos: Vec<i32>,
        }
        let value = Nested {
            inner: Inner { x: "y", n: -5 },
            flag: 1,
            pos: vec![1, 2, 3],
        };
        assert_eq!(hash(&value).unwrap(), NESTED);
    }

    #[test]
    fn map_entries_sort_as_unsigned() {
        assert_eq!(hash(&"a").unwrap(), KEY_A);
        assert_eq!(hash(&"h").unwrap(), KEY_H);
        assert_eq!(hash(&"p").unwrap(), KEY_P);
        const { assert!(KEY_H < KEY_P && KEY_P < KEY_A) };
        const { assert!((KEY_P as u32) < (KEY_A as u32) && (KEY_A as u32) < (KEY_H as u32)) };
        let sixteen: BTreeMap<String, i32> =
            ('a'..='p').map(|c| (c.to_string(), c as i32)).collect();
        assert_eq!(hash(&sixteen).unwrap(), SIXTEEN_KEYS);
        let mixed = map(&[("k", "v"), ("k2", "v2"), ("count", "1"), ("id", "x")]);
        assert_eq!(hash(&mixed).unwrap(), MIXED_SIGN_KEYS);
    }

    #[test]
    fn map_hash_ignores_insertion_order() {
        #[derive(Serialize)]
        struct Forward {
            a: i32,
            b: i32,
        }
        #[derive(Serialize)]
        struct Backward {
            b: i32,
            a: i32,
        }
        assert_eq!(
            hash(&Forward { a: 1, b: 2 }).unwrap(),
            hash(&Backward { b: 2, a: 1 }).unwrap()
        );
    }

    #[test]
    fn absent_values_are_omitted_from_maps_like_nbt() {
        #[derive(Serialize)]
        struct Optional {
            a: Option<i32>,
            b: (),
        }
        assert_eq!(hash(&Optional { a: None, b: () }).unwrap(), EMPTY_MAP);
        assert_eq!(hash(&Optional { a: Some(1), b: () }).unwrap(), A_INT_1);
        assert_eq!(
            hash(&map(&[("a", Some(1i32)), ("b", None)])).unwrap(),
            A_INT_1
        );
    }

    #[test]
    fn absent_list_elements_hash_as_empty() {
        assert_eq!(hash(&[None::<i32>]).unwrap(), hash(&[()]).unwrap());
        assert_ne!(hash(&[None::<i32>]).unwrap(), EMPTY_LIST);
    }

    #[test]
    fn serde_forms_follow_the_nbt_serializer() {
        #[derive(Serialize)]
        enum Kind {
            Foo,
            Wrapped(i32),
            Pair(i32, i32),
            Record { a: i32 },
        }
        #[derive(Serialize)]
        struct Newtype(i32);
        #[derive(Serialize)]
        struct Tuple(i32, i32);
        #[derive(Serialize)]
        struct Unit;
        struct RawBytes;
        impl Serialize for RawBytes {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_bytes(&[1, 2, 253])
            }
        }

        assert_eq!(hash(&Kind::Foo).unwrap(), hash(&"Foo").unwrap());
        assert_eq!(hash(&200u8).unwrap(), hash(&-56i8).unwrap());
        assert_eq!(hash(&RawBytes).unwrap(), hash(&[1i8, 2, -3]).unwrap());
        assert!(hash(&'c').is_err());
        assert!(hash(&1u16).is_err());
        assert!(hash(&1u32).is_err());
        assert!(hash(&1u64).is_err());
        assert!(hash(&Newtype(1)).is_err());
        assert!(hash(&Tuple(1, 2)).is_err());
        assert!(hash(&Unit).is_err());
        assert!(hash(&Kind::Wrapped(1)).is_err());
        assert!(hash(&Kind::Pair(1, 2)).is_err());
        assert!(hash(&Kind::Record { a: 1 }).is_err());
        assert!(hash(&BTreeMap::from([((), 1i32)])).is_err());
    }

    #[test]
    fn is_not_human_readable() {
        assert!(!Serializer::is_human_readable(&HashSerializer));
    }
}
