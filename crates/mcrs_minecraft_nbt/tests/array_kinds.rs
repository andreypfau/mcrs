use std::io::Cursor;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{ArrayKind, ArrayVisitor, Nbt, from_bytes, from_tag, nbt_array};
use serde::Deserialize;

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
