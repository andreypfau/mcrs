use alloc::vec::Vec;
use core::error::Error;
use core::fmt;

use mcrs_minecraft_nbt::ArrayKind;
use mcrs_minecraft_nbt::NBT_ARRAY_TAG;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::to_nbt_tag;
use serde::Serialize;
use serde::ser;
use serde::ser::Impossible;

use crate::node_hash::ListHash;
use crate::node_hash::MapEntry;
use crate::node_hash::NodeHash;

/// The hash of the tree that `value` serializes to: a boolean is a boolean node and each number
/// keeps its kind, which the NBT form of the tree loses. A map sorts its entries in `entries` and
/// leaves it as it found it.
pub(crate) fn hash_of<T: Serialize + ?Sized>(
    value: &T,
    entries: &mut Vec<MapEntry>,
) -> Result<NodeHash, Unhashable> {
    value
        .serialize(NodeSerializer { entries })?
        .ok_or(Unhashable)
}

/// The value serializes to a shape that has no node.
#[derive(Debug)]
pub(crate) struct Unhashable;

impl fmt::Display for Unhashable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the value has no node of the saved form")
    }
}

impl Error for Unhashable {}

impl ser::Error for Unhashable {
    fn custom<T: fmt::Display>(_message: T) -> Self {
        Self
    }
}

// `None` is an absent value, which a map omits as the saved form does.
type Node = Option<NodeHash>;

struct NodeSerializer<'scratch> {
    entries: &'scratch mut Vec<MapEntry>,
}

impl<'scratch> ser::Serializer for NodeSerializer<'scratch> {
    type Ok = Node;
    type Error = Unhashable;
    type SerializeSeq = ListSerializer<'scratch>;
    type SerializeTuple = ListSerializer<'scratch>;
    type SerializeTupleStruct = ListSerializer<'scratch>;
    type SerializeTupleVariant = Impossible<Node, Unhashable>;
    type SerializeMap = MapSerializer<'scratch>;
    type SerializeStruct = MapSerializer<'scratch>;
    type SerializeStructVariant = Impossible<Node, Unhashable>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, value: bool) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::boolean(value)))
    }

    fn serialize_i8(self, value: i8) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::byte(value)))
    }

    fn serialize_i16(self, value: i16) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::short(value)))
    }

    fn serialize_i32(self, value: i32) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::int(value)))
    }

    fn serialize_i64(self, value: i64) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::long(value)))
    }

    fn serialize_u8(self, value: u8) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::byte(value.cast_signed())))
    }

    fn serialize_u16(self, value: u16) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::short(value.cast_signed())))
    }

    fn serialize_u32(self, value: u32) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::int(value.cast_signed())))
    }

    fn serialize_u64(self, value: u64) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::long(value.cast_signed())))
    }

    fn serialize_f32(self, value: f32) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::float(value)))
    }

    fn serialize_f64(self, value: f64) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::double(value)))
    }

    fn serialize_char(self, _value: char) -> Result<Node, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_str(self, value: &str) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::string(value)))
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Node, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_none(self) -> Result<Node, Unhashable> {
        Ok(None)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Node, Unhashable> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Node, Unhashable> {
        Ok(None)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Node, Unhashable> {
        Ok(None)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Node, Unhashable> {
        Ok(Some(NodeHash::string(variant)))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Node, Unhashable> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Node, Unhashable> {
        if name != NBT_ARRAY_TAG {
            return Err(Unhashable);
        }
        array_hash(variant, value).map(Some)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<ListSerializer<'scratch>, Unhashable> {
        Ok(ListSerializer {
            entries: self.entries,
            list: ListHash::new(),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<ListSerializer<'scratch>, Unhashable> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<ListSerializer<'scratch>, Unhashable> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<Node, Unhashable>, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<MapSerializer<'scratch>, Unhashable> {
        Ok(MapSerializer {
            start: self.entries.len(),
            entries: self.entries,
            key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<MapSerializer<'scratch>, Unhashable> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<Node, Unhashable>, Unhashable> {
        Err(Unhashable)
    }
}

// An array occurs only in a UUID and in a payload, so the detour through the NBT tree costs little.
fn array_hash<T: Serialize + ?Sized>(variant: &str, value: &T) -> Result<NodeHash, Unhashable> {
    let NbtTag::List(elements) = to_nbt_tag(&value).map_err(|_| Unhashable)? else {
        return Err(Unhashable);
    };
    let kind = [ArrayKind::Byte, ArrayKind::Int, ArrayKind::Long]
        .into_iter()
        .find(|kind| kind.variant() == variant)
        .ok_or(Unhashable)?;
    match kind {
        ArrayKind::Byte => {
            let values = array_values(elements, |tag| match tag {
                NbtTag::Byte(value) => Some(value),
                _ => None,
            })?;
            Ok(NodeHash::byte_array(&values))
        }
        ArrayKind::Int => {
            let values = array_values(elements, |tag| match tag {
                NbtTag::Int(value) => Some(value),
                _ => None,
            })?;
            Ok(NodeHash::int_array(&values))
        }
        ArrayKind::Long => {
            let values = array_values(elements, |tag| match tag {
                NbtTag::Long(value) => Some(value),
                _ => None,
            })?;
            Ok(NodeHash::long_array(&values))
        }
    }
}

fn array_values<V>(
    elements: Vec<NbtTag>,
    value: fn(NbtTag) -> Option<V>,
) -> Result<Vec<V>, Unhashable> {
    (elements.into_iter())
        .map(|element| value(element).ok_or(Unhashable))
        .collect()
}

struct ListSerializer<'scratch> {
    entries: &'scratch mut Vec<MapEntry>,
    list: ListHash,
}

impl ListSerializer<'_> {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Unhashable> {
        let element = hash_of(value, self.entries)?;
        self.list = self.list.push(element);
        Ok(())
    }
}

macro_rules! list_serializer {
    ($($trait:ident :: $method:ident),*) => {$(
        impl ser::$trait for ListSerializer<'_> {
            type Ok = Node;
            type Error = Unhashable;

            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Unhashable> {
                self.push(value)
            }

            fn end(self) -> Result<Node, Unhashable> {
                Ok(Some(self.list.finish()))
            }
        }
    )*};
}

list_serializer!(
    SerializeSeq::serialize_element,
    SerializeTuple::serialize_element,
    SerializeTupleStruct::serialize_field
);

// The entries of the map stand in `entries` from `start` on. A nested map adds its entries after
// them and removes them when it ends, so one buffer serves each level.
struct MapSerializer<'scratch> {
    entries: &'scratch mut Vec<MapEntry>,
    start: usize,
    key: Option<NodeHash>,
}

impl MapSerializer<'_> {
    fn put<T: Serialize + ?Sized>(&mut self, key: NodeHash, value: &T) -> Result<(), Unhashable> {
        if let Some(value) = value.serialize(NodeSerializer {
            entries: self.entries,
        })? {
            self.entries.push(MapEntry { key, value });
        }
        Ok(())
    }

    fn finish(self) -> Node {
        let hash = NodeHash::map(self.entries.get_mut(self.start..).unwrap_or_default());
        self.entries.truncate(self.start);
        Some(hash)
    }
}

impl ser::SerializeMap for MapSerializer<'_> {
    type Ok = Node;
    type Error = Unhashable;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Unhashable> {
        self.key = Some(hash_of(key, self.entries)?);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Unhashable> {
        let key = self.key.take().ok_or(Unhashable)?;
        self.put(key, value)
    }

    fn end(self) -> Result<Node, Unhashable> {
        Ok(self.finish())
    }
}

impl ser::SerializeStruct for MapSerializer<'_> {
    type Ok = Node;
    type Error = Unhashable;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Unhashable> {
        self.put(NodeHash::string(key), value)
    }

    fn end(self) -> Result<Node, Unhashable> {
        Ok(self.finish())
    }
}
