#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no component type of this crate has a node of each kind yet"
    )
)]

use mcrs_minecraft_nbt::ArrayKind;

/// The hash of one node of the saved form of a component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NodeHash(u32);

/// One entry of a map node. The order is the order of the entries in the hash of the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MapEntry {
    pub(crate) key: NodeHash,
    pub(crate) value: NodeHash,
}

impl NodeHash {
    #[must_use]
    pub(crate) fn absent() -> Self {
        Checksum::tagged(0x01).finish()
    }

    #[must_use]
    pub(crate) fn byte(value: i8) -> Self {
        Checksum::tagged(0x06).push(&value.to_le_bytes()).finish()
    }

    #[must_use]
    pub(crate) fn short(value: i16) -> Self {
        Checksum::tagged(0x07).push(&value.to_le_bytes()).finish()
    }

    #[must_use]
    pub(crate) fn int(value: i32) -> Self {
        Checksum::tagged(0x08).push(&value.to_le_bytes()).finish()
    }

    #[must_use]
    pub(crate) fn long(value: i64) -> Self {
        Checksum::tagged(0x09).push(&value.to_le_bytes()).finish()
    }

    #[must_use]
    pub(crate) fn float(value: f32) -> Self {
        Checksum::tagged(0x0A)
            .push(&value.to_bits().to_le_bytes())
            .finish()
    }

    #[must_use]
    pub(crate) fn double(value: f64) -> Self {
        Checksum::tagged(0x0B)
            .push(&value.to_bits().to_le_bytes())
            .finish()
    }

    #[must_use]
    pub(crate) fn string(value: &str) -> Self {
        Self::string_of_parts(&[value])
    }

    /// The hash of the string node that is the concatenation of `parts`.
    #[must_use]
    pub(crate) fn string_of_parts(parts: &[&str]) -> Self {
        let units: usize = parts.iter().map(|part| part.encode_utf16().count()).sum();
        let units = u32::try_from(units).unwrap_or(u32::MAX);
        let mut checksum = Checksum::tagged(0x0C).push(&units.to_le_bytes());
        for part in parts {
            for unit in part.encode_utf16() {
                checksum = checksum.push(&unit.to_le_bytes());
            }
        }
        checksum.finish()
    }

    #[must_use]
    pub(crate) fn boolean(value: bool) -> Self {
        Checksum::tagged(0x0D).push(&[u8::from(value)]).finish()
    }

    #[must_use]
    pub(crate) fn byte_array(values: &[i8]) -> Self {
        let mut array = ArrayHash::new(ArrayKind::Byte);
        for value in values {
            array = array.push(&value.to_le_bytes());
        }
        array.finish()
    }

    #[must_use]
    pub(crate) fn int_array(values: &[i32]) -> Self {
        let mut array = ArrayHash::new(ArrayKind::Int);
        for value in values {
            array = array.push(&value.to_le_bytes());
        }
        array.finish()
    }

    #[must_use]
    pub(crate) fn long_array(values: &[i64]) -> Self {
        let mut array = ArrayHash::new(ArrayKind::Long);
        for value in values {
            array = array.push(&value.to_le_bytes());
        }
        array.finish()
    }

    /// The hash of a list node from the hashes of its elements in order.
    #[must_use]
    pub(crate) fn list(elements: impl IntoIterator<Item = Self>) -> Self {
        let mut list = ListHash::new();
        for element in elements {
            list = list.push(element);
        }
        list.finish()
    }

    /// The hash of a map node. Sorts `entries` in place; the input order has no effect.
    #[must_use]
    pub(crate) fn map(entries: &mut [MapEntry]) -> Self {
        entries.sort_unstable();
        let mut checksum = Checksum::tagged(0x02);
        for entry in entries.iter() {
            checksum = checksum
                .push(&entry.key.0.to_le_bytes())
                .push(&entry.value.0.to_le_bytes());
        }
        checksum.push(&[0x03]).finish()
    }

    /// The value that the hashed stack carries on the wire.
    #[must_use]
    pub(crate) fn as_i32(self) -> i32 {
        self.0.cast_signed()
    }
}

/// A list node whose elements come one at a time, for elements whose hash can fail.
#[derive(Clone, Copy)]
pub(crate) struct ListHash(Checksum);

impl ListHash {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self(Checksum::tagged(0x04))
    }

    #[must_use]
    pub(crate) fn push(self, element: NodeHash) -> Self {
        Self(self.0.push(&element.0.to_le_bytes()))
    }

    #[must_use]
    pub(crate) fn finish(self) -> NodeHash {
        self.0.push(&[0x05]).finish()
    }
}

/// An array node whose elements come one at a time. Each element is pushed in the little-endian
/// bytes of the kind of the array.
#[derive(Clone, Copy)]
pub(crate) struct ArrayHash {
    checksum: Checksum,
    end: u8,
}

impl ArrayHash {
    #[must_use]
    pub(crate) fn new(kind: ArrayKind) -> Self {
        let (start, end) = match kind {
            ArrayKind::Byte => (0x0E, 0x0F),
            ArrayKind::Int => (0x10, 0x11),
            ArrayKind::Long => (0x12, 0x13),
        };
        Self {
            checksum: Checksum::tagged(start),
            end,
        }
    }

    #[must_use]
    pub(crate) fn push(self, element: &[u8]) -> Self {
        Self {
            checksum: self.checksum.push(element),
            end: self.end,
        }
    }

    #[must_use]
    pub(crate) fn finish(self) -> NodeHash {
        self.checksum.push(&[self.end]).finish()
    }
}

#[derive(Clone, Copy)]
struct Checksum(u32);

impl Checksum {
    const REFLECTED_POLYNOMIAL: u32 = 0x82F6_3B78;

    fn tagged(tag: u8) -> Self {
        Self(u32::MAX).push(&[tag])
    }

    // chisle: one bit per step. A 256-entry table lifts the cost when a profile shows this hash.
    fn push(mut self, bytes: &[u8]) -> Self {
        for &byte in bytes {
            self.0 ^= u32::from(byte);
            for _ in 0..8 {
                let low_bit_mask = (self.0 & 1).wrapping_neg();
                self.0 = (self.0 >> 1) ^ (Self::REFLECTED_POLYNOMIAL & low_bit_mask);
            }
        }
        self
    }

    fn finish(self) -> NodeHash {
        NodeHash(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;
    use mcrs_minecraft_fixtures::fixture;

    enum Node {
        Absent,
        Byte(i8),
        Short(i16),
        Int(i32),
        Long(i64),
        Float(f32),
        Double(f64),
        Text(&'static str),
        Boolean(bool),
        ByteArray(&'static [i8]),
        IntArray(&'static [i32]),
        LongArray(&'static [i64]),
        List(&'static [Node]),
        Map(&'static [(&'static str, Node)]),
    }

    fn hash(node: &Node) -> NodeHash {
        match node {
            Node::Absent => NodeHash::absent(),
            Node::Byte(value) => NodeHash::byte(*value),
            Node::Short(value) => NodeHash::short(*value),
            Node::Int(value) => NodeHash::int(*value),
            Node::Long(value) => NodeHash::long(*value),
            Node::Float(value) => NodeHash::float(*value),
            Node::Double(value) => NodeHash::double(*value),
            Node::Text(value) => NodeHash::string(value),
            Node::Boolean(value) => NodeHash::boolean(*value),
            Node::ByteArray(values) => NodeHash::byte_array(values),
            Node::IntArray(values) => NodeHash::int_array(values),
            Node::LongArray(values) => NodeHash::long_array(values),
            Node::List(elements) => NodeHash::list(elements.iter().map(hash)),
            Node::Map(fields) => {
                let mut entries: Vec<MapEntry> = fields
                    .iter()
                    .map(|(key, value)| MapEntry {
                        key: NodeHash::string(key),
                        value: hash(value),
                    })
                    .collect();
                NodeHash::map(&mut entries)
            }
        }
    }

    #[test]
    fn a_node_has_the_hash_that_the_game_recorded() {
        let recorded: BTreeMap<String, i32> = serde_json::from_str(&fixture("hash_ops").text())
            .expect("the fixture is a map of case names to hashes");
        let cases: &[(&str, Node)] = &[
            ("empty", Node::Absent),
            ("empty_map", Node::Map(&[])),
            ("empty_list", Node::List(&[])),
            ("a_byte_1", Node::Map(&[("a", Node::Byte(1))])),
            ("a_int_1", Node::Map(&[("a", Node::Int(1))])),
            ("a_short_neg2", Node::Map(&[("a", Node::Short(-2))])),
            ("str_empty", Node::Text("")),
            ("str_hello", Node::Text("hello")),
            ("str_astral", Node::Text("a\u{1F600}b")),
            (
                "list_ints",
                Node::List(&[Node::Int(1), Node::Int(-2), Node::Int(300)]),
            ),
            (
                "nested",
                Node::Map(&[
                    (
                        "inner",
                        Node::Map(&[("x", Node::Text("y")), ("n", Node::Long(-5))]),
                    ),
                    ("flag", Node::Byte(1)),
                    ("pos", Node::IntArray(&[1, 2, 3])),
                ]),
            ),
            ("int_array", Node::IntArray(&[7, -8, 9])),
            ("long_array", Node::LongArray(&[1 << 40, -1])),
            ("byte_array", Node::ByteArray(&[1, 2, -3])),
            ("float", Node::Float(1.5)),
            ("double", Node::Double(-2.25)),
            ("long", Node::Long(1_234_567_890_123)),
            ("bool_true", Node::Boolean(true)),
            ("bool_false", Node::Boolean(false)),
            ("byte_true", Node::Byte(1)),
            (
                "list_of_bools",
                Node::List(&[Node::Boolean(true), Node::Boolean(false)]),
            ),
            (
                "sixteen_keys",
                Node::Map(&[
                    ("a", Node::Int(97)),
                    ("b", Node::Int(98)),
                    ("c", Node::Int(99)),
                    ("d", Node::Int(100)),
                    ("e", Node::Int(101)),
                    ("f", Node::Int(102)),
                    ("g", Node::Int(103)),
                    ("h", Node::Int(104)),
                    ("i", Node::Int(105)),
                    ("j", Node::Int(106)),
                    ("k", Node::Int(107)),
                    ("l", Node::Int(108)),
                    ("m", Node::Int(109)),
                    ("n", Node::Int(110)),
                    ("o", Node::Int(111)),
                    ("p", Node::Int(112)),
                ]),
            ),
            (
                "mixed_sign_keys",
                Node::Map(&[
                    ("k", Node::Text("v")),
                    ("k2", Node::Text("v2")),
                    ("count", Node::Text("1")),
                    ("id", Node::Text("x")),
                ]),
            ),
            ("key_a", Node::Text("a")),
            ("key_h", Node::Text("h")),
            ("key_p", Node::Text("p")),
        ];
        let covered: Vec<&str> = cases.iter().map(|(name, _)| *name).collect();
        let fixture_names: Vec<&str> = recorded.keys().map(String::as_str).collect();
        let mut sorted_covered = covered.clone();
        sorted_covered.sort_unstable();
        assert_eq!(
            sorted_covered, fixture_names,
            "each fixture case has exactly one input"
        );
        for (name, node) in cases {
            assert_eq!(
                Some(&hash(node).as_i32()),
                recorded.get(*name),
                "the case {name}"
            );
        }
    }
}
