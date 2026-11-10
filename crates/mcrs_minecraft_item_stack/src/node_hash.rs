#![expect(
    dead_code,
    reason = "the codec of the hashed stack is the first caller"
)]

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
        let units = u32::try_from(value.encode_utf16().count()).unwrap_or(u32::MAX);
        let mut checksum = Checksum::tagged(0x0C).push(&units.to_le_bytes());
        for unit in value.encode_utf16() {
            checksum = checksum.push(&unit.to_le_bytes());
        }
        checksum.finish()
    }

    #[must_use]
    pub(crate) fn boolean(value: bool) -> Self {
        Checksum::tagged(0x0D).push(&[u8::from(value)]).finish()
    }

    #[must_use]
    pub(crate) fn byte_array(values: &[i8]) -> Self {
        let mut checksum = Checksum::tagged(0x0E);
        for value in values {
            checksum = checksum.push(&value.to_le_bytes());
        }
        checksum.push(&[0x0F]).finish()
    }

    #[must_use]
    pub(crate) fn int_array(values: &[i32]) -> Self {
        let mut checksum = Checksum::tagged(0x10);
        for value in values {
            checksum = checksum.push(&value.to_le_bytes());
        }
        checksum.push(&[0x11]).finish()
    }

    #[must_use]
    pub(crate) fn long_array(values: &[i64]) -> Self {
        let mut checksum = Checksum::tagged(0x12);
        for value in values {
            checksum = checksum.push(&value.to_le_bytes());
        }
        checksum.push(&[0x13]).finish()
    }

    /// The hash of a list node from the hashes of its elements in order.
    #[must_use]
    pub(crate) fn list(elements: impl IntoIterator<Item = Self>) -> Self {
        let mut checksum = Checksum::tagged(0x04);
        for element in elements {
            checksum = checksum.push(&element.0.to_le_bytes());
        }
        checksum.push(&[0x05]).finish()
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
        Text(&'static str),
        Boolean(bool),
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
            Node::Text(value) => NodeHash::string(value),
            Node::Boolean(value) => NodeHash::boolean(*value),
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
            ("float", Node::Float(1.5)),
            ("long", Node::Long(1_234_567_890_123)),
            ("bool_true", Node::Boolean(true)),
            ("bool_false", Node::Boolean(false)),
            ("byte_true", Node::Byte(1)),
            (
                "list_of_bools",
                Node::List(&[Node::Boolean(true), Node::Boolean(false)]),
            ),
            ("key_a", Node::Text("a")),
            ("key_h", Node::Text("h")),
            ("key_p", Node::Text("p")),
        ];
        for (name, node) in cases {
            assert_eq!(
                Some(&hash(node).as_i32()),
                recorded.get(*name),
                "the case {name}"
            );
        }
    }
}
