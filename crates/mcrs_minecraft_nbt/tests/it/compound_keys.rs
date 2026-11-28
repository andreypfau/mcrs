use std::io::Cursor;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::tag::NbtTag;

fn entry(tag_id: u8, name: &str, payload: &[u8]) -> Vec<u8> {
    let length = u16::try_from(name.len()).unwrap().to_be_bytes();
    [&[tag_id][..], &length, name.as_bytes(), payload].concat()
}

#[test]
fn a_repeated_key_keeps_its_first_position_and_takes_its_last_value() {
    let byte = 1;
    let string = 8;
    let bytes = [
        vec![10],
        entry(byte, "b", &[1]),
        entry(byte, "a", &[2]),
        entry(byte, "b", &[3]),
        entry(byte, "c", &[4]),
        entry(string, "a", &[0, 1, b'x']),
        entry(byte, "b", &[5]),
        vec![0],
    ]
    .concat();
    let decoded = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(bytes))).unwrap();
    let mut expected = NbtCompound::new();
    expected.child_tags = vec![
        ("b".to_owned(), NbtTag::Byte(5)),
        ("a".to_owned(), NbtTag::String("x".to_owned())),
        ("c".to_owned(), NbtTag::Byte(4)),
    ];
    assert_eq!(decoded, NbtTag::Compound(expected));
}
