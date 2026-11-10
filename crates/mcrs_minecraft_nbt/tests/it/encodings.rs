use std::fmt::Debug;
use std::io::Cursor;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{
    BigEndian, Error, LittleEndian, Nbt, NbtEncoding, NetworkLittleEndian, from_bytes_unnamed_with,
    from_bytes_with, to_bytes_unnamed_with, to_bytes_with,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Player {
    name: String,
    score: i32,
    xp: i64,
    health: f32,
    mana: f64,
    active: bool,
    inventory: Vec<String>,
    bytes: Vec<u8>,
    ints: Vec<i32>,
    longs: Vec<i64>,
}

fn round_trips<E: NbtEncoding, T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
    let mut named = Vec::new();
    to_bytes_with::<E, T>(value, &mut named).unwrap();
    assert_eq!(&from_bytes_with::<E, T>(Cursor::new(named)).unwrap(), value);

    let mut unnamed = Vec::new();
    to_bytes_unnamed_with::<E, T>(value, &mut unnamed).unwrap();
    assert_eq!(
        &from_bytes_unnamed_with::<E, T>(Cursor::new(unnamed)).unwrap(),
        value
    );
}

#[test]
fn a_typed_value_round_trips_in_each_encoding() {
    let player = Player {
        name: "Steve 🌍".to_string(),
        score: 12345,
        xp: 9876543210,
        health: 19.5,
        mana: 99.99,
        active: true,
        inventory: vec!["sword".to_string(), "shield".to_string()],
        bytes: vec![0, 1, 127, 128, 255],
        ints: vec![-1000000, -1, 0, 1, i32::MAX, i32::MIN],
        longs: vec![-999999999999, -1, 0, 1, i64::MAX, i64::MIN],
    };

    round_trips::<BigEndian, _>(&player);
    round_trips::<LittleEndian, _>(&player);
    round_trips::<NetworkLittleEndian, _>(&player);
}

fn every_tag_kind() -> NbtCompound {
    let mut inner = NbtCompound::new();
    inner.put_string("inner", "value".to_string());

    let mut all = NbtCompound::new();
    all.put_byte("byte", -42);
    all.put_short("short", -1234);
    all.put_int("int", i32::MIN);
    all.put_long("long", i64::MIN);
    all.put_float("float", 1.5);
    all.put_double("double", -2.25);
    all.put_string("string", "é 🌍".to_string());
    all.put(
        "bytes",
        NbtTag::ByteArray(vec![0xDE, 0xAD].into_boxed_slice()),
    );
    all.put("ints", NbtTag::IntArray(vec![-5, 0, i32::MAX, i32::MIN]));
    all.put("longs", NbtTag::LongArray(vec![-5, 0, i64::MAX, i64::MIN]));
    all.put(
        "list",
        NbtTag::List(vec![NbtTag::Long(300), NbtTag::Long(-300)]),
    );
    all.put_component("compound", inner);
    all
}

fn read_unnamed<E: NbtEncoding>(bytes: &[u8]) -> Result<Nbt, Error> {
    Nbt::read_unnamed_with(&mut NbtReadHelper::<_, E>::with_encoding(Cursor::new(
        bytes,
    )))
}

#[test]
fn a_tag_tree_round_trips_in_each_encoding() {
    fn check<E: NbtEncoding>() {
        let nbt = Nbt::new("root".to_string(), every_tag_kind());

        let named = nbt.write_with::<E>().unwrap();
        let mut reader = NbtReadHelper::<_, E>::with_encoding(Cursor::new(&named));
        assert_eq!(Nbt::read_with(&mut reader).unwrap(), nbt);

        let unnamed = nbt.write_unnamed_with::<E>().unwrap();
        assert_eq!(read_unnamed::<E>(&unnamed).unwrap().root_tag, nbt.root_tag);
    }

    check::<BigEndian>();
    check::<LittleEndian>();
    check::<NetworkLittleEndian>();
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Wide {
    before: i32,
    skipped: NbtTag,
    after: i32,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Narrow {
    before: i32,
    after: i32,
}

#[test]
fn an_unknown_field_of_any_tag_kind_is_skipped_in_each_encoding() {
    fn check<E: NbtEncoding>() {
        let wide = Wide {
            before: -7,
            skipped: NbtTag::Compound(every_tag_kind()),
            after: 7,
        };
        let mut bytes = Vec::new();
        to_bytes_unnamed_with::<E, _>(&wide, &mut bytes).unwrap();

        let narrow: Narrow = from_bytes_unnamed_with::<E, _>(Cursor::new(bytes)).unwrap();
        assert_eq!(
            narrow,
            Narrow {
                before: -7,
                after: 7
            }
        );
    }

    check::<BigEndian>();
    check::<LittleEndian>();
    check::<NetworkLittleEndian>();
}

struct Wire {
    tag: NbtTag,
    big_endian: &'static [u8],
    little_endian: &'static [u8],
    network: &'static [u8],
}

/// Each payload follows the tag id and the name `k` of a one-field compound.
#[test]
fn each_encoding_has_its_own_bytes_on_the_wire() {
    let cases = [
        Wire {
            tag: NbtTag::Short(0x1234),
            big_endian: &[0x12, 0x34],
            little_endian: &[0x34, 0x12],
            network: &[0x34, 0x12],
        },
        Wire {
            tag: NbtTag::Int(-1),
            big_endian: &[0xFF, 0xFF, 0xFF, 0xFF],
            little_endian: &[0xFF, 0xFF, 0xFF, 0xFF],
            network: &[0x01],
        },
        Wire {
            tag: NbtTag::Int(64),
            big_endian: &[0, 0, 0, 0x40],
            little_endian: &[0x40, 0, 0, 0],
            network: &[0x80, 0x01],
        },
        Wire {
            tag: NbtTag::Int(i32::MIN),
            big_endian: &[0x80, 0, 0, 0],
            little_endian: &[0, 0, 0, 0x80],
            network: &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F],
        },
        Wire {
            tag: NbtTag::Long(-2),
            big_endian: &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE],
            little_endian: &[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            network: &[0x03],
        },
        Wire {
            tag: NbtTag::Long(i64::MAX),
            big_endian: &[0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            little_endian: &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
            network: &[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
        },
        Wire {
            tag: NbtTag::Float(1.0),
            big_endian: &[0x3F, 0x80, 0, 0],
            little_endian: &[0, 0, 0x80, 0x3F],
            network: &[0, 0, 0x80, 0x3F],
        },
        Wire {
            tag: NbtTag::String("é".to_string()),
            big_endian: &[0, 2, 0xC3, 0xA9],
            little_endian: &[2, 0, 0xC3, 0xA9],
            network: &[2, 0xC3, 0xA9],
        },
        Wire {
            tag: NbtTag::IntArray(vec![1, -1]),
            big_endian: &[0, 0, 0, 2, 0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF],
            little_endian: &[2, 0, 0, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
            network: &[0x04, 0x02, 0x01],
        },
        Wire {
            tag: NbtTag::LongArray(vec![-1]),
            big_endian: &[0, 0, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            little_endian: &[1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            network: &[0x02, 0x01],
        },
    ];

    fn check<E: NbtEncoding>(tag: &NbtTag, name: &[u8], payload: &[u8]) {
        let mut compound = NbtCompound::new();
        compound.put("k", tag.clone());
        let nbt = Nbt::new(String::new(), compound);

        let mut expected = vec![0x0A, tag.get_type_id()];
        expected.extend_from_slice(name);
        expected.extend_from_slice(payload);
        expected.push(0);

        assert_eq!(
            nbt.write_unnamed_with::<E>().unwrap().as_ref(),
            expected,
            "{tag:?}"
        );
        assert_eq!(read_unnamed::<E>(&expected).unwrap(), nbt, "{tag:?}");
    }

    for case in &cases {
        check::<BigEndian>(&case.tag, &[0, 1, b'k'], case.big_endian);
        check::<LittleEndian>(&case.tag, &[1, 0, b'k'], case.little_endian);
        check::<NetworkLittleEndian>(&case.tag, &[1, b'k'], case.network);
    }
}

#[test]
fn malformed_input_is_an_error_a_caller_can_match() {
    let negative_length =
        read_unnamed::<LittleEndian>(&[0x0A, 0x07, 1, 0, b'k', 0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(matches!(negative_length, Err(Error::NegativeLength(-1))));

    let negative_varint_length = read_unnamed::<NetworkLittleEndian>(&[0x0A, 0x07, 1, b'k', 0x01]);
    assert!(matches!(
        negative_varint_length,
        Err(Error::NegativeLength(-1))
    ));

    let invalid_utf8 = read_unnamed::<LittleEndian>(&[0x0A, 0x08, 1, 0, b'k', 1, 0, 0xFF]);
    assert!(matches!(invalid_utf8, Err(Error::InvalidUtf8)));

    for (tag, varint) in [
        (0x03, &[0xFF, 0xFF, 0xFF, 0xFF, 0x1F][..]),
        (0x03, &[0xFF, 0xFF, 0xFF, 0xFF, 0x8F, 0x00]),
        (
            0x04,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x02],
        ),
    ] {
        let mut bytes = vec![0x0A, tag, 1, b'k'];
        bytes.extend_from_slice(varint);
        bytes.push(0);
        let read = read_unnamed::<NetworkLittleEndian>(&bytes);
        assert!(matches!(read, Err(Error::MalformedVarInt)), "{varint:02X?}");
    }
}
