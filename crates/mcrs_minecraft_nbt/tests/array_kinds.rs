use std::fmt;
use std::io::Cursor;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{ArrayKind, ArrayVisitor, Nbt, from_bytes, from_tag, nbt_array};
use serde::Deserialize;
use serde::de::{DeserializeOwned, Visitor};

#[derive(Debug, PartialEq)]
enum Seen {
    Array(ArrayKind, Vec<u8>),
    Other(NbtTag),
}

impl<'de> Deserialize<'de> for Seen {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct See;

        impl<'de> ArrayVisitor<'de> for See {
            type Value = Seen;

            fn visit_array<E>(self, kind: ArrayKind, payload: &[u8]) -> Result<Seen, E> {
                Ok(Seen::Array(kind, payload.to_vec()))
            }

            fn visit_other<D: serde::Deserializer<'de>>(self, value: D) -> Result<Seen, D::Error> {
                Ok(Seen::Other(<NbtTag as Deserialize>::deserialize(value)?))
            }
        }

        nbt_array(deserializer, See)
    }
}

#[derive(Deserialize, Debug, PartialEq)]
struct Holder {
    data: Seen,
    after: i32,
}

fn holding(value: NbtTag) -> NbtCompound {
    let mut root = NbtCompound::new();
    root.put("data", value);
    root.put_int("after", 7);
    root
}

/// What the value reads as from a file and from a tag held in memory.
fn seen(value: NbtTag) -> [Seen; 2] {
    let root = holding(value);
    let bytes = Nbt::new(String::new(), root.clone()).write();
    let stored: Holder = from_bytes(Cursor::new(&bytes[..])).unwrap();
    let held: Holder = from_tag(NbtTag::Compound(root)).unwrap();
    assert_eq!((stored.after, held.after), (7, 7));
    [stored.data, held.data]
}

#[test]
fn an_array_arrives_whole_with_the_kind_of_its_elements() {
    let arrays = [
        (
            NbtTag::ByteArray(Box::new([1, 2, 0xff])),
            ArrayKind::Byte,
            vec![1, 2, 0xff],
        ),
        (
            NbtTag::IntArray(vec![1, -2]),
            ArrayKind::Int,
            vec![0, 0, 0, 1, 0xff, 0xff, 0xff, 0xfe],
        ),
        (
            NbtTag::LongArray(vec![i64::MIN, 9]),
            ArrayKind::Long,
            vec![0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9],
        ),
        (NbtTag::LongArray(Vec::new()), ArrayKind::Long, Vec::new()),
    ];
    for (value, kind, payload) in arrays {
        for read in seen(value.clone()) {
            assert_eq!(read, Seen::Array(kind, payload.clone()), "{value:?}");
        }
    }
}

#[test]
fn a_value_that_is_no_array_is_handed_over_unread() {
    let others = [
        NbtTag::List(vec![NbtTag::Long(1), NbtTag::Long(2)]),
        NbtTag::List(vec![NbtTag::Byte(1), NbtTag::Byte(2)]),
        NbtTag::List(Vec::new()),
        NbtTag::Int(7),
        NbtTag::String("seven".to_string()),
        NbtTag::Compound(holding(NbtTag::Double(0.5))),
    ];
    for value in others {
        for read in seen(value.clone()) {
            assert_eq!(read, Seen::Other(value.clone()));
        }
    }
}

#[test]
fn an_unsigned_byte_is_read_from_bytes_and_from_nothing_wider() {
    #[derive(Deserialize, Debug)]
    struct Unsigned {
        data: Vec<u8>,
    }

    let read = |value| {
        let bytes = Nbt::new(String::new(), holding(value)).write();
        from_bytes::<Unsigned>(Cursor::new(&bytes[..]))
    };

    let bytes = read(NbtTag::List(vec![NbtTag::Byte(1), NbtTag::Byte(-1)]));
    assert_eq!(bytes.unwrap().data, [1, 0xff]);
    let bytes = read(NbtTag::ByteArray(Box::new([1, 0xff])));
    assert_eq!(bytes.unwrap().data, [1, 0xff]);

    let wider = [
        NbtTag::List(vec![NbtTag::Long(0); 4]),
        NbtTag::List(vec![NbtTag::Short(1)]),
        NbtTag::IntArray(vec![0, 0]),
        NbtTag::LongArray(vec![0]),
    ];
    for value in wider {
        let read = read(value.clone());
        assert!(read.is_err(), "{value:?} read as {read:?}");
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
enum Status {
    #[serde(rename = "minecraft:full")]
    Full,
    A,
}

#[derive(Debug, PartialEq)]
struct Name(String);

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Named;

        impl Visitor<'_> for Named {
            type Value = Name;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a name")
            }

            fn visit_str<E>(self, name: &str) -> Result<Name, E> {
                Ok(Name(name.to_string()))
            }
        }

        deserializer.deserialize_identifier(Named)
    }
}

#[derive(Deserialize, Debug, PartialEq)]
struct Stored<T> {
    data: T,
    after: i32,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Tolerant {
    #[serde(deserialize_with = "status_or_none")]
    data: Option<Status>,
    after: i32,
}

fn status_or_none<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Status>, D::Error> {
    Ok(Status::deserialize(deserializer).ok())
}

/// The read from a file and the read from a tag held in memory, each as its
/// value or the text of its error.
fn both<T: DeserializeOwned>(value: NbtTag) -> [Result<T, String>; 2] {
    let root = holding(value);
    let bytes = Nbt::new(String::new(), root.clone()).write();
    [
        from_bytes(Cursor::new(&bytes[..])).map_err(|error| error.to_string()),
        from_tag(NbtTag::Compound(root)).map_err(|error| error.to_string()),
    ]
}

fn assert_the_same_type_error<T: DeserializeOwned + fmt::Debug + PartialEq>(value: NbtTag) {
    let [stored, held] = both::<T>(value.clone());
    let error = stored.expect_err(&format!("{value:?} read from a file"));
    assert!(error.contains("invalid type"), "{value:?}: {error}");
    assert_eq!(Err(error), held, "{value:?}");
}

#[test]
fn a_string_names_an_enum_variant() {
    for (name, status) in [("minecraft:full", Status::Full), ("A", Status::A)] {
        for read in both::<Stored<Status>>(NbtTag::String(name.to_string())) {
            let expected = Stored {
                data: status,
                after: 7,
            };
            assert_eq!(read, Ok(expected), "{name}");
        }
    }

    let [stored, held] = both::<Stored<Status>>(NbtTag::String("minecraft:fuller".to_string()));
    let error = stored.unwrap_err();
    assert!(error.contains("unknown variant"), "{error}");
    assert_eq!(Err(error), held);
}

#[test]
fn an_enum_stored_as_a_number_is_a_type_error_and_the_fields_after_it_still_read() {
    // Taken as a string, the second int is a length of one, the letter `A`
    // and a zero that closes the enclosing compound.
    let numbers = [
        NbtTag::Int(7),
        NbtTag::Int(0x0001_4100),
        NbtTag::Byte(1),
        NbtTag::Long(0x0001_4100_0000_0000),
        NbtTag::Double(0.5),
    ];
    for value in numbers {
        assert_the_same_type_error::<Stored<Status>>(value.clone());

        for read in both::<Tolerant>(value.clone()) {
            let passed_over = Tolerant {
                data: None,
                after: 7,
            };
            assert_eq!(read, Ok(passed_over), "{value:?}");
        }
    }
}

#[test]
fn an_enum_stored_as_an_array_declaring_more_bytes_than_the_input_holds_is_an_error() {
    // Taken as a string, the declared count is a length of 14 followed by
    // `mi`, so the field spells `minecraft:full` and the fields after it are
    // read out of what the header calls the array's payload.
    let mut bytes = vec![0x0a, 0, 0];
    bytes.extend([0x07, 0, 4]);
    bytes.extend(b"data");
    bytes.extend(0x000e_6d69_i32.to_be_bytes());
    bytes.extend(b"necraft:full");
    bytes.extend([0x03, 0, 5]);
    bytes.extend(b"after");
    bytes.extend(7_i32.to_be_bytes());
    bytes.push(0);

    let error = from_bytes::<Stored<Status>>(Cursor::new(&bytes[..]))
        .expect_err("an array of 945,513 bytes with 12 present")
        .to_string();
    assert!(error.contains("invalid type"), "{error}");
    let tolerated = from_bytes::<Tolerant>(Cursor::new(&bytes[..]));
    assert!(tolerated.is_err(), "{tolerated:?}");

    let held = holding(NbtTag::ByteArray(Box::new(*b"necraft:full")));
    let held = from_tag::<Stored<Status>>(NbtTag::Compound(held));
    assert_eq!(error, held.unwrap_err().to_string());
}

#[test]
fn an_enum_stored_as_a_collection_is_the_same_type_error_from_a_file_and_from_memory() {
    let collections = [
        NbtTag::ByteArray(Box::new(*b"\0\x01A")),
        NbtTag::ByteArray(Box::new([])),
        NbtTag::IntArray(vec![0x0001_4100]),
        NbtTag::LongArray(vec![1]),
        NbtTag::List(vec![NbtTag::String("A".to_string())]),
        NbtTag::List(Vec::new()),
        NbtTag::Compound(holding(NbtTag::String("A".to_string()))),
        NbtTag::Compound(NbtCompound::new()),
    ];
    for value in collections {
        assert_the_same_type_error::<Stored<Status>>(value);
    }
}

#[test]
fn an_identifier_is_read_from_a_string_and_from_no_other_tag() {
    for read in both::<Stored<Name>>(NbtTag::String("A".to_string())) {
        let expected = Stored {
            data: Name("A".to_string()),
            after: 7,
        };
        assert_eq!(read, Ok(expected));
    }

    let others = [
        NbtTag::Int(0x0001_4100),
        NbtTag::ByteArray(Box::new(*b"\0\x01A")),
        NbtTag::List(vec![NbtTag::String("A".to_string())]),
        NbtTag::Compound(NbtCompound::new()),
    ];
    for value in others {
        assert_the_same_type_error::<Stored<Name>>(value);
    }
}
