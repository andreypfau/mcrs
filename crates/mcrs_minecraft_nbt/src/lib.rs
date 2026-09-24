use std::{
    fmt::Display,
    io::{self, Read, Seek},
};

use bytes::Bytes;
use compound::NbtCompound;
use deserializer::NbtReadHelper;
use serde::{de, ser};
use serializer::WriteAdaptor;
use tag::NbtTag;
use thiserror::Error;

pub mod compound;
pub mod deserializer;
pub mod nbt_compress;
pub mod serializer;
pub mod snbt;
#[cfg(test)]
mod snbt_golden;
pub mod tag;
pub mod tag_deserializer;
pub mod tag_serializer;

pub use deserializer::{from_bytes, from_bytes_unnamed};
pub use serializer::{to_bytes, to_bytes_named, to_bytes_unnamed};

pub use tag_deserializer::from_tag;
pub use tag_serializer::{to_nbt_compound, to_nbt_tag};

thread_local! {
    static BINARY_READS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// serde's buffering deserializers (`flatten`, `tag`, `untagged`) replay a
/// value through one that reports itself human-readable, which would make a
/// tag read from NBT below them look JSON-sourced and get its numbers
/// narrowed. A live NBT deserializer on the thread settles the question.
#[derive(Debug)]
pub(crate) struct BinaryReadGuard;

impl BinaryReadGuard {
    pub(crate) fn new() -> Self {
        BINARY_READS.with(|reads| reads.set(reads.get() + 1));
        BinaryReadGuard
    }
}

impl Drop for BinaryReadGuard {
    fn drop(&mut self) {
        BINARY_READS.with(|reads| reads.set(reads.get() - 1));
    }
}

pub(crate) fn reading_binary() -> bool {
    BINARY_READS.with(|reads| reads.get() > 0)
}

// This NBT crate is inspired from CrabNBT

pub const END_ID: u8 = 0x00;
pub const BYTE_ID: u8 = 0x01;
pub const SHORT_ID: u8 = 0x02;
pub const INT_ID: u8 = 0x03;
pub const LONG_ID: u8 = 0x04;
pub const FLOAT_ID: u8 = 0x05;
pub const DOUBLE_ID: u8 = 0x06;
pub const BYTE_ARRAY_ID: u8 = 0x07;
pub const STRING_ID: u8 = 0x08;
pub const LIST_ID: u8 = 0x09;
pub const COMPOUND_ID: u8 = 0x0A;
pub const INT_ARRAY_ID: u8 = 0x0B;
pub const LONG_ARRAY_ID: u8 = 0x0C;

#[derive(Error, Debug)]
pub enum Error {
    #[error("The root tag of the NBT file is not a compound tag. Received tag id: {0}")]
    NoRootCompound(u8),
    #[error("The root tag is TAG_End")]
    EndRoot,
    #[error("SNBT syntax error at {position}: {message}")]
    Snbt { position: usize, message: String },
    #[error("Encountered an unknown NBT tag id: {0}.")]
    UnknownTagId(u8),
    #[error("Failed to Cesu 8 Decode")]
    Cesu8DecodingError,
    #[error("Serde error: {0}")]
    SerdeError(String),
    #[error("NBT doesn't support this type: {0}")]
    UnsupportedType(String),
    #[error("NBT reading was cut short: {0}")]
    Incomplete(io::Error),
    #[error("Negative list length: {0}")]
    NegativeLength(i32),
    #[error("Length too large: {0}")]
    LargeLength(usize),
    #[error("Tried to read NBT tag with too high complexity, depth > {MAX_DEPTH}")]
    TooDeep,
}

/// `NbtAccounter.defaultQuota`: lists and compounds may nest this far.
pub const MAX_DEPTH: u32 = 512;

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::SerdeError(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::SerdeError(msg.to_string())
    }
}

#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Nbt {
    pub name: String,
    pub root_tag: NbtCompound,
}

impl Nbt {
    pub fn new(name: String, tag: NbtCompound) -> Self {
        Nbt {
            name,
            root_tag: tag,
        }
    }

    pub fn read<R: Read + Seek>(reader: &mut NbtReadHelper<R>) -> Result<Nbt, Error> {
        let tag_type_id = reader.get_u8_be()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Nbt {
            name: get_nbt_string(reader)?,
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    /// Reads an NBT tag that doesn't contain the name of the root `Compound`.
    pub fn read_unnamed<R: Read + Seek>(reader: &mut NbtReadHelper<R>) -> Result<Nbt, Error> {
        let tag_type_id = reader.get_u8_be()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Nbt {
            name: String::new(),
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    pub fn write(&self) -> Bytes {
        let mut bytes = Vec::new();
        let mut writer = WriteAdaptor::new(&mut bytes);
        writer.write_u8_be(COMPOUND_ID).unwrap();
        NbtTag::String(self.name.to_string())
            .serialize_data(&mut writer)
            .unwrap();
        self.root_tag.serialize_content(&mut writer).unwrap();

        bytes.into()
    }

    /// Writes an NBT tag without a root `Compound` name.
    pub fn write_unnamed(&self) -> Bytes {
        let mut bytes = Vec::new();
        let mut writer = WriteAdaptor::new(&mut bytes);

        writer.write_u8_be(COMPOUND_ID).unwrap();
        self.root_tag.serialize_content(&mut writer).unwrap();

        bytes.into()
    }
}

pub fn get_nbt_string<R: Read + Seek>(bytes: &mut NbtReadHelper<R>) -> Result<String, Error> {
    let len = bytes.get_u16_be()? as usize;
    let string_bytes = bytes.read_boxed_slice(len)?;
    let string = cesu8::from_java_cesu8(&string_bytes).map_err(|_| Error::Cesu8DecodingError)?;
    Ok(string.to_string())
}

// TODO: This is a bit hacky
pub(crate) const NBT_ARRAY_TAG: &str = "__nbt_array";
pub(crate) const NBT_INT_ARRAY_TAG: &str = "__nbt_int_array";
pub(crate) const NBT_LONG_ARRAY_TAG: &str = "__nbt_long_array";
pub(crate) const NBT_BYTE_ARRAY_TAG: &str = "__nbt_byte_array";

macro_rules! impl_array {
    ($name:ident, $variant:expr) => {
        pub fn $name<T: serde::Serialize, S: serde::Serializer>(
            input: T,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            serializer.serialize_newtype_variant(NBT_ARRAY_TAG, 0, $variant, &input)
        }
    };
}

impl_array!(nbt_int_array, NBT_INT_ARRAY_TAG);
impl_array!(nbt_long_array, NBT_LONG_ARRAY_TAG);
impl_array!(nbt_byte_array, NBT_BYTE_ARRAY_TAG);

/// NBT has no boolean, so a flag is a byte. A tagged enum buffers the compound
/// before it knows the variant, and a buffered byte never reaches
/// `deserialize_bool`, so a `bool` field inside one reads through this.
pub fn nbt_flag<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    struct Flag;
    impl serde::de::Visitor<'_> for Flag {
        type Value = bool;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a boolean or the byte standing for one")
        }

        fn visit_bool<E>(self, value: bool) -> Result<bool, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<bool, E> {
            Ok(value != 0)
        }

        fn visit_u64<E>(self, value: u64) -> Result<bool, E> {
            Ok(value != 0)
        }
    }
    deserializer.deserialize_any(Flag)
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use crate::Error;
    use crate::deserializer::from_bytes;
    use crate::nbt_byte_array;
    use crate::nbt_int_array;
    use crate::nbt_long_array;
    use crate::serializer::to_bytes;
    use crate::serializer::to_bytes_named;
    use crate::{Nbt, compound, tag};
    use crate::{deserializer::from_bytes_unnamed, serializer::to_bytes_unnamed};
    use serde::{Deserialize, Serialize};

    /// A list of compounds carries anything that is not one wrapped as
    /// `{"": value}`, and a name saved without properties reaches the palette
    /// that way.
    #[test]
    fn a_wrapped_list_element_reads_as_the_value_it_wraps() {
        #[derive(Deserialize, PartialEq, Debug)]
        #[serde(untagged)]
        enum Entry {
            Name(String),
            State { id: String },
        }

        let mut state = compound::NbtCompound::new();
        state.put_string("id", "minecraft:glow_lichen".to_string());

        let mut root = compound::NbtCompound::new();
        root.put_list(
            "palette",
            vec![
                tag::NbtTag::String("minecraft:deepslate".to_string()),
                tag::NbtTag::Compound(state),
            ],
        );

        #[derive(Deserialize, PartialEq, Debug)]
        struct Palette {
            palette: Vec<Entry>,
        }

        let bytes = Nbt::new(String::new(), root).write().to_vec();
        let read: Palette = from_bytes(Cursor::new(bytes)).unwrap();
        assert_eq!(
            read.palette,
            vec![
                Entry::Name("minecraft:deepslate".to_string()),
                Entry::State {
                    id: "minecraft:glow_lichen".to_string()
                },
            ]
        );
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Test {
        byte: i8,
        short: i16,
        int: i32,
        long: i64,
        float: f32,
        string: String,
    }

    #[test]
    fn test_simple_ser_de_unnamed() {
        let test = Test {
            byte: 123,
            short: 1342,
            int: 4313,
            long: 34,
            float: 1.00,
            string: "Hello test".to_string(),
        };

        let mut bytes = Vec::new();
        to_bytes_unnamed(&test, &mut bytes).unwrap();
        let recreated_struct: Test = from_bytes_unnamed(Cursor::new(bytes)).unwrap();

        assert_eq!(test, recreated_struct);
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestArray {
        #[serde(serialize_with = "nbt_byte_array")]
        byte_array: Vec<u8>,
        #[serde(serialize_with = "nbt_int_array")]
        int_array: Vec<i32>,
        #[serde(serialize_with = "nbt_long_array")]
        long_array: Vec<i64>,
    }

    #[test]
    fn test_simple_ser_de_array() {
        let test = TestArray {
            byte_array: vec![0, 3, 2],
            int_array: vec![13, 1321, 2],
            long_array: vec![1, 0, 200301, 1],
        };

        let mut bytes = Vec::new();
        to_bytes_unnamed(&test, &mut bytes).unwrap();
        let recreated_struct: TestArray = from_bytes_unnamed(Cursor::new(bytes)).unwrap();

        assert_eq!(test, recreated_struct);
    }

    #[test]
    fn test_simple_ser_de_named() {
        let name = String::from("Test");
        let test = Test {
            byte: 123,
            short: 1342,
            int: 4313,
            long: 34,
            float: 1.00,
            string: "Hello test".to_string(),
        };

        let mut bytes = Vec::new();
        to_bytes_named(&test, name, &mut bytes).unwrap();
        let recreated_struct: Test = from_bytes(Cursor::new(bytes)).unwrap();

        assert_eq!(test, recreated_struct);
    }

    #[test]
    fn test_simple_ser_de_array_named() {
        let name = String::from("Test");
        let test = TestArray {
            byte_array: vec![0, 3, 2],
            int_array: vec![13, 1321, 2],
            long_array: vec![1, 0, 200301, 1],
        };

        let mut bytes = Vec::new();
        to_bytes_named(&test, name, &mut bytes).unwrap();
        let recreated_struct: TestArray = from_bytes(Cursor::new(bytes)).unwrap();

        assert_eq!(test, recreated_struct);
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Egg {
        food: String,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Breakfast {
        food: Egg,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestList {
        option: Option<Egg>,
        nested_compound: Breakfast,
        compounds: Vec<Test>,
        list_string: Vec<String>,
        empty: Vec<Test>,
    }

    #[test]
    fn test_list() {
        let test1 = Test {
            byte: 123,
            short: 1342,
            int: 4313,
            long: 34,
            float: 1.00,
            string: "Hello test".to_string(),
        };

        let test2 = Test {
            byte: 13,
            short: 342,
            int: -4313,
            long: -132334,
            float: -69.420,
            string: "Hello compounds".to_string(),
        };

        let list_compound = TestList {
            option: Some(Egg {
                food: "Skibid".to_string(),
            }),
            nested_compound: Breakfast {
                food: Egg {
                    food: "Over easy".to_string(),
                },
            },
            compounds: vec![test1, test2],
            list_string: vec!["".to_string(), "abcbcbcbbc".to_string()],
            empty: vec![],
        };

        let mut bytes = Vec::new();
        to_bytes_unnamed(&list_compound, &mut bytes).unwrap();
        let recreated_struct: TestList = from_bytes_unnamed(Cursor::new(bytes)).unwrap();
        assert_eq!(list_compound, recreated_struct);
    }

    #[test]
    fn test_list_named() {
        let test1 = Test {
            byte: 123,
            short: 1342,
            int: 4313,
            long: 34,
            float: 1.00,
            string: "Hello test".to_string(),
        };

        let test2 = Test {
            byte: 13,
            short: 342,
            int: -4313,
            long: -132334,
            float: -69.420,
            string: "Hello compounds".to_string(),
        };

        let list_compound = TestList {
            option: None,
            nested_compound: Breakfast {
                food: Egg {
                    food: "Over easy".to_string(),
                },
            },
            compounds: vec![test1, test2],
            list_string: vec!["".to_string(), "abcbcbcbbc".to_string()],
            empty: vec![],
        };

        let mut bytes = Vec::new();
        to_bytes_named(&list_compound, "a".to_string(), &mut bytes).unwrap();
        let recreated_struct: TestList = from_bytes(Cursor::new(bytes)).unwrap();
        assert_eq!(list_compound, recreated_struct);
    }

    #[test]
    fn test_nbt_arrays() {
        #[derive(Serialize)]
        struct Tagged {
            #[serde(serialize_with = "nbt_long_array")]
            l: [i64; 1],
            #[serde(serialize_with = "nbt_int_array")]
            i: [i32; 1],
            #[serde(serialize_with = "nbt_byte_array")]
            b: [u8; 1],
        }

        let value = Tagged {
            l: [0],
            i: [0],
            b: [0],
        };
        let expected_bytes = [
            0x0A, // Component Tag
            0x00, 0x00, // Empty root name
            0x0C, // Long Array Type
            0x00, 0x01, // Key length
            0x6C, // Key (l)
            0x00, 0x00, 0x00, 0x01, // Array Length
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Value(s)
            0x0B, // Int Array Tag
            0x00, 0x01, // Key length
            0x69, // Key (i)
            0x00, 0x00, 0x00, 0x01, // Array Length
            0x00, 0x00, 0x00, 0x00, // Value(s)
            0x07, // Byte Array Tag
            0x00, 0x01, // Key length
            0x62, // Key (b)
            0x00, 0x00, 0x00, 0x01, // Array Length
            0x00, // Value(s)
            0x00, // End Tag
        ];

        let mut bytes = Vec::new();
        to_bytes(&value, &mut bytes).unwrap();
        assert_eq!(bytes, expected_bytes);

        #[derive(Serialize)]
        struct NotTagged {
            l: [i64; 1],
            i: [i32; 1],
            b: [u8; 1],
        }

        let value = NotTagged {
            l: [0],
            i: [0],
            b: [0],
        };
        let expected_bytes = [
            0x0A, // Component Tag
            0x00, 0x00, // Empty root name
            0x09, // List Tag
            0x00, 0x01, // Key length
            0x6C, // Key (l)
            0x04, // Array Type
            0x00, 0x00, 0x00, 0x01, // Array Length
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Value(s)
            0x09, // List Tag
            0x00, 0x01, // Key length
            0x69, // Key (i)
            0x03, // Array Type
            0x00, 0x00, 0x00, 0x01, // Array Length
            0x00, 0x00, 0x00, 0x00, // Value(s)
            0x09, // List Tag
            0x00, 0x01, // Key length
            0x62, // Key (b)
            0x01, // Array Type
            0x00, 0x00, 0x00, 0x01, // Array Length
            0x00, // Value(s)
            0x00, // End Tag
        ];

        let mut bytes = Vec::new();
        to_bytes(&value, &mut bytes).unwrap();
        assert_eq!(bytes, expected_bytes);
    }

    pub(crate) fn unhex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Vanilla bytes of `{l:["a",{x:1b},5]}`: element type 10 with every
    /// non-compound element wrapped as `{"": value}`.
    const MIXED_LIST_HEX: &str = "0a0900016c0a0000000308000000016100010001780100030000000000050000";

    #[test]
    fn a_mixed_list_writes_and_reads_like_vanilla() {
        #[derive(Serialize)]
        struct X {
            x: i8,
        }
        #[derive(Serialize)]
        struct Mixed {
            l: (String, X, i32),
        }

        let value = Mixed {
            l: ("a".to_string(), X { x: 1 }, 5),
        };
        let mut bytes = Vec::new();
        to_bytes_unnamed(&value, &mut bytes).unwrap();
        assert_eq!(bytes, unhex(MIXED_LIST_HEX));

        let mut x = compound::NbtCompound::new();
        x.put_byte("x", 1);
        let list = tag::NbtTag::List(vec![
            tag::NbtTag::String("a".to_string()),
            tag::NbtTag::Compound(x),
            tag::NbtTag::Int(5),
        ]);
        let read: compound::NbtCompound = from_bytes_unnamed(Cursor::new(&bytes)).unwrap();
        assert_eq!(read.get("l"), Some(&list));
        assert_eq!(
            crate::to_nbt_compound(&value).unwrap().get("l"),
            Some(&list)
        );

        let mut written = Vec::new();
        to_bytes_unnamed(&read, &mut written).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn a_wrapper_compound_survives_a_list_round_trip() {
        let mut wrapper = compound::NbtCompound::new();
        wrapper.put_string("", "deepslate".to_string());
        let mut plain = compound::NbtCompound::new();
        plain.put_string("id", "lichen".to_string());
        let list = tag::NbtTag::List(vec![
            tag::NbtTag::Compound(wrapper),
            tag::NbtTag::Compound(plain),
        ]);

        let mut bytes = Vec::new();
        to_bytes_unnamed(&list, &mut bytes).unwrap();
        assert_eq!(
            bytes,
            unhex("090a000000020a0000080000000964656570736c6174650000080002696400066c696368656e00")
        );
        assert_eq!(
            from_bytes_unnamed::<tag::NbtTag>(Cursor::new(bytes)).unwrap(),
            list
        );
    }

    #[test]
    fn an_unnamed_root_may_be_any_tag_but_end() {
        let recipes = vec!["minecraft:a".to_string(), "minecraft:b".to_string()];
        let mut bytes = Vec::new();
        to_bytes_unnamed(&recipes, &mut bytes).unwrap();
        assert_eq!(
            bytes,
            unhex("090800000002000b6d696e6563726166743a61000b6d696e6563726166743a62")
        );
        assert_eq!(
            from_bytes_unnamed::<Vec<String>>(Cursor::new(&bytes)).unwrap(),
            recipes
        );
        assert_eq!(
            from_bytes_unnamed::<tag::NbtTag>(Cursor::new(&bytes)).unwrap(),
            tag::NbtTag::List(
                recipes
                    .iter()
                    .map(|s| tag::NbtTag::String(s.clone()))
                    .collect()
            )
        );

        let mut bytes = Vec::new();
        to_bytes_unnamed(&"{x:1b}", &mut bytes).unwrap();
        assert_eq!(bytes, unhex("0800067b783a31627d"));
        assert_eq!(
            from_bytes_unnamed::<String>(Cursor::new(&bytes)).unwrap(),
            "{x:1b}"
        );

        let mut bytes = Vec::new();
        to_bytes_unnamed(&-7i32, &mut bytes).unwrap();
        assert_eq!(bytes, unhex("03fffffff9"));
        assert_eq!(from_bytes_unnamed::<i32>(Cursor::new(&bytes)).unwrap(), -7);
        assert!(from_bytes_unnamed::<Test>(Cursor::new(&bytes)).is_err());

        assert!(matches!(
            to_bytes_unnamed(&None::<i32>, &mut Vec::new()),
            Err(Error::EndRoot)
        ));
        assert!(matches!(
            from_bytes_unnamed::<tag::NbtTag>(Cursor::new([0x00])),
            Err(Error::EndRoot)
        ));
        assert!(matches!(
            to_bytes(&-7i32, &mut Vec::new()),
            Err(Error::NoRootCompound(0x03))
        ));
        assert!(matches!(
            from_bytes::<i32>(Cursor::new(unhex("03fffffff9"))),
            Err(Error::NoRootCompound(0x03))
        ));
    }

    #[test]
    fn json_numbers_are_typed_like_json_ops() {
        for (json, expected) in crate::snbt_golden::JSON_TYPING {
            let tag: tag::NbtTag = serde_json::from_str(json).unwrap();
            assert_eq!(crate::snbt::write(&tag), *expected, "{json}");
        }
        let nbt_long: tag::NbtTag =
            from_bytes_unnamed(Cursor::new(unhex("040000000000000001"))).unwrap();
        assert_eq!(nbt_long, tag::NbtTag::Long(1));
        assert_eq!(
            crate::from_tag::<tag::NbtTag>(tag::NbtTag::Double(1.0)).unwrap(),
            tag::NbtTag::Double(1.0)
        );
    }

    #[test]
    fn test_tuple_ok() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct GoodData {
            x: (i32, i32),
        }

        let value = GoodData { x: (1, 2) };
        let mut bytes = Vec::new();
        to_bytes(&value, &mut bytes).unwrap();

        let reconstructed = from_bytes(Cursor::new(bytes)).unwrap();
        assert_eq!(value, reconstructed);
    }

    fn compound_with_arrays() -> compound::NbtCompound {
        let mut root = compound::NbtCompound::new();
        root.put("bytes", tag::NbtTag::ByteArray(Box::new([1, 2, 255])));
        root.put("ints", tag::NbtTag::IntArray(vec![-1, 0, i32::MAX]));
        root.put("longs", tag::NbtTag::LongArray(vec![i64::MIN, 7]));
        root.put_list("list", vec![tag::NbtTag::Int(1), tag::NbtTag::Int(2)]);
        root
    }

    #[test]
    fn a_compound_keeps_its_array_tags_through_serde() {
        let root = compound_with_arrays();
        let bytes = Nbt::new(String::new(), root.clone()).write();

        let read: compound::NbtCompound = from_bytes(Cursor::new(bytes.clone())).unwrap();
        assert_eq!(read, root);
        assert_eq!(crate::to_nbt_compound(&read).unwrap(), root);

        let mut written = Vec::new();
        to_bytes(&root, &mut written).unwrap();
        assert_eq!(written, bytes.to_vec());
    }

    #[test]
    fn a_list_of_arrays_keeps_its_array_tags_through_serde() {
        let mut root = compound::NbtCompound::new();
        root.put_list(
            "byte_arrays",
            vec![
                tag::NbtTag::ByteArray(Box::new([1, 2])),
                tag::NbtTag::ByteArray(Box::new([])),
            ],
        );
        root.put_list(
            "int_arrays",
            vec![
                tag::NbtTag::IntArray(vec![-1, i32::MAX]),
                tag::NbtTag::IntArray(vec![3]),
            ],
        );
        root.put_list(
            "long_arrays",
            vec![
                tag::NbtTag::LongArray(vec![i64::MIN]),
                tag::NbtTag::LongArray(vec![7, 8]),
            ],
        );
        let bytes = Nbt::new(String::new(), root.clone()).write();

        let mut written = Vec::new();
        to_bytes(&root, &mut written).unwrap();
        assert_eq!(written, bytes.to_vec());

        let read: compound::NbtCompound = from_bytes(Cursor::new(bytes)).unwrap();
        assert_eq!(read, root);
        assert_eq!(crate::to_nbt_compound(&read).unwrap(), root);
    }

    #[test]
    fn a_struct_holding_a_compound_keeps_its_array_tags() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Holder {
            id: i32,
            nbt: compound::NbtCompound,
        }

        let value = Holder {
            id: 3,
            nbt: compound_with_arrays(),
        };
        let mut bytes = Vec::new();
        to_bytes(&value, &mut bytes).unwrap();

        let read: Holder = from_bytes(Cursor::new(bytes)).unwrap();
        assert_eq!(read, value);
        let in_memory = crate::to_nbt_compound(&read).unwrap();
        assert_eq!(
            in_memory.get("nbt"),
            Some(&tag::NbtTag::Compound(value.nbt))
        );
    }

    #[test]
    fn arrays_are_plain_lists_in_json() {
        let json = serde_json::to_string(&compound_with_arrays()).unwrap();
        assert_eq!(
            json,
            r#"{"bytes":[1,2,-1],"ints":[-1,0,2147483647],"longs":[-9223372036854775808,7],"list":[1,2]}"#
        );
    }

    #[test]
    fn a_buffered_tag_keeps_its_binary_widths() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        #[serde(tag = "id")]
        enum Tagged {
            A {
                data: compound::NbtCompound,
                payload: tag::NbtTag,
            },
        }

        let mut data = compound::NbtCompound::new();
        data.put("l", tag::NbtTag::Long(1));
        data.put("d", tag::NbtTag::Double(1.0));
        data.put("i", tag::NbtTag::Int(5));
        let value = Tagged::A {
            data,
            payload: tag::NbtTag::Long(-1),
        };

        let mut bytes = Vec::new();
        to_bytes_unnamed(&value, &mut bytes).unwrap();
        assert_eq!(
            from_bytes_unnamed::<Tagged>(Cursor::new(&bytes)).unwrap(),
            value
        );
        let tag = crate::to_nbt_compound(&value).unwrap();
        assert_eq!(
            crate::from_tag::<Tagged>(tag::NbtTag::Compound(tag)).unwrap(),
            value
        );

        let json: Tagged =
            serde_json::from_str(r#"{"id":"A","data":{"l":1,"d":1.0,"i":5},"payload":-1}"#)
                .unwrap();
        let Tagged::A { data, payload } = json;
        assert_eq!(data.get("l"), Some(&tag::NbtTag::Byte(1)));
        assert_eq!(data.get("d"), Some(&tag::NbtTag::Byte(1)));
        assert_eq!(payload, tag::NbtTag::Byte(-1));
    }

    #[test]
    fn nesting_stops_at_the_vanilla_depth() {
        let nested = |depth: usize| {
            let mut bytes = vec![crate::LIST_ID];
            for _ in 0..depth {
                bytes.extend_from_slice(&[crate::LIST_ID, 0, 0, 0, 1]);
            }
            bytes.extend_from_slice(&[crate::END_ID, 0, 0, 0, 0]);
            bytes
        };
        assert!(from_bytes_unnamed::<tag::NbtTag>(Cursor::new(nested(510))).is_ok());
        assert!(matches!(
            from_bytes_unnamed::<tag::NbtTag>(Cursor::new(nested(2000))),
            Err(Error::TooDeep)
        ));

        #[derive(Deserialize)]
        struct Nothing {}
        let mut skipped = vec![crate::COMPOUND_ID, crate::LIST_ID, 0, 1, b'a'];
        skipped.extend_from_slice(&nested(2000)[1..]);
        skipped.push(crate::END_ID);
        assert!(matches!(
            from_bytes_unnamed::<Nothing>(Cursor::new(skipped)),
            Err(Error::TooDeep)
        ));
    }

    #[test]
    fn a_json_array_still_reads_as_a_list() {
        let tag: tag::NbtTag = serde_json::from_value(serde_json::json!([1, 2, 3])).unwrap();
        assert_eq!(
            tag,
            tag::NbtTag::List(vec![
                tag::NbtTag::Byte(1),
                tag::NbtTag::Byte(2),
                tag::NbtTag::Byte(3)
            ])
        );
    }
}
