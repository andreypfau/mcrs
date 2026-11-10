use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use std::hash::{BuildHasher, RandomState};

#[derive(Clone, Copy, Debug)]
enum Expected {
    Equal,
    Unequal,
}

fn compound(tag: NbtTag) -> NbtTag {
    let mut compound = NbtCompound::default();
    compound.child_tags.push(("value".to_owned(), tag));
    NbtTag::Compound(compound)
}

#[test]
fn numbers_compare_by_tag_kind_and_bit_pattern() {
    let quiet_float = f32::from_bits(0x7fc0_0000);
    let payload_float = f32::from_bits(0xffa0_0001);
    let quiet_double = f64::from_bits(0x7ff8_0000_0000_0000);
    let payload_double = f64::from_bits(0xfff4_0000_0000_0001);
    let cases = [
        (
            NbtTag::Float(quiet_float),
            NbtTag::Float(payload_float),
            Expected::Equal,
        ),
        (
            NbtTag::Double(quiet_double),
            NbtTag::Double(payload_double),
            Expected::Equal,
        ),
        (NbtTag::Float(1.5), NbtTag::Float(1.5), Expected::Equal),
        (NbtTag::Double(-0.0), NbtTag::Double(-0.0), Expected::Equal),
        (NbtTag::Float(0.0), NbtTag::Float(-0.0), Expected::Unequal),
        (NbtTag::Double(0.0), NbtTag::Double(-0.0), Expected::Unequal),
        (NbtTag::Float(1.0), NbtTag::Double(1.0), Expected::Unequal),
        (NbtTag::Int(1), NbtTag::Byte(1), Expected::Unequal),
    ];
    let hasher = RandomState::new();
    for (left, right, expected) in cases {
        let wrapped = [
            (left.clone(), right.clone()),
            (
                NbtTag::List(vec![left.clone()]),
                NbtTag::List(vec![right.clone()]),
            ),
            (compound(left), compound(right)),
        ];
        for (left, right) in wrapped {
            match expected {
                Expected::Equal => {
                    assert_eq!(left, right);
                    assert_eq!(
                        hasher.hash_one(&left),
                        hasher.hash_one(&right),
                        "{left:?} and {right:?} are equal, so their hashes are equal"
                    );
                }
                Expected::Unequal => assert_ne!(left, right),
            }
        }
    }
}
