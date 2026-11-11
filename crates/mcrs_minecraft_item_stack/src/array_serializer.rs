use mcrs_minecraft_nbt::ArrayKind;
use serde::Serialize;
use serde::ser;
use serde::ser::Impossible;

use crate::node_hash::ArrayHash;
use crate::node_hash::NodeHash;
use crate::node_serializer::Unhashable;

/// The hash of the array node of the kind that `variant` names, with the elements that `value`
/// serializes to. The elements go into the checksum as they come.
pub(crate) fn array_hash<T: Serialize + ?Sized>(
    variant: &str,
    value: &T,
) -> Result<NodeHash, Unhashable> {
    let kind = [ArrayKind::Byte, ArrayKind::Int, ArrayKind::Long]
        .into_iter()
        .find(|kind| kind.variant() == variant)
        .ok_or(Unhashable)?;
    let array = value.serialize(ArraySerializer {
        kind,
        part: ArrayPart::Whole,
        array: ArrayHash::new(kind),
    })?;
    Ok(array.finish())
}

#[derive(Clone, Copy, PartialEq)]
enum ArrayPart {
    Whole,
    Element,
}

struct ArraySerializer {
    kind: ArrayKind,
    part: ArrayPart,
    array: ArrayHash,
}

impl ArraySerializer {
    fn element(self, kind: ArrayKind, bytes: &[u8]) -> Result<ArrayHash, Unhashable> {
        if self.part == ArrayPart::Element && self.kind == kind {
            Ok(self.array.push(bytes))
        } else {
            Err(Unhashable)
        }
    }

    fn elements(self) -> Result<Self, Unhashable> {
        if self.part == ArrayPart::Whole {
            Ok(self)
        } else {
            Err(Unhashable)
        }
    }

    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Unhashable> {
        self.array = value.serialize(ArraySerializer {
            kind: self.kind,
            part: ArrayPart::Element,
            array: self.array,
        })?;
        Ok(())
    }
}

impl ser::Serializer for ArraySerializer {
    type Ok = ArrayHash;
    type Error = Unhashable;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Impossible<ArrayHash, Unhashable>;
    type SerializeMap = Impossible<ArrayHash, Unhashable>;
    type SerializeStruct = Impossible<ArrayHash, Unhashable>;
    type SerializeStructVariant = Impossible<ArrayHash, Unhashable>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, _value: bool) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_i8(self, value: i8) -> Result<ArrayHash, Unhashable> {
        self.element(ArrayKind::Byte, &value.to_le_bytes())
    }

    fn serialize_i16(self, _value: i16) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_i32(self, value: i32) -> Result<ArrayHash, Unhashable> {
        self.element(ArrayKind::Int, &value.to_le_bytes())
    }

    fn serialize_i64(self, value: i64) -> Result<ArrayHash, Unhashable> {
        self.element(ArrayKind::Long, &value.to_le_bytes())
    }

    fn serialize_u8(self, value: u8) -> Result<ArrayHash, Unhashable> {
        self.element(ArrayKind::Byte, &value.to_le_bytes())
    }

    fn serialize_u16(self, _value: u16) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_u32(self, value: u32) -> Result<ArrayHash, Unhashable> {
        self.element(ArrayKind::Int, &value.to_le_bytes())
    }

    fn serialize_u64(self, value: u64) -> Result<ArrayHash, Unhashable> {
        self.element(ArrayKind::Long, &value.to_le_bytes())
    }

    fn serialize_f32(self, _value: f32) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_f64(self, _value: f64) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_char(self, _value: char) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_str(self, _value: &str) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_none(self) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<ArrayHash, Unhashable> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<ArrayHash, Unhashable> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<ArrayHash, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self, Unhashable> {
        self.elements()
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self, Unhashable> {
        self.elements()
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Self, Unhashable> {
        self.elements()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<ArrayHash, Unhashable>, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_map(
        self,
        _len: Option<usize>,
    ) -> Result<Impossible<ArrayHash, Unhashable>, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Impossible<ArrayHash, Unhashable>, Unhashable> {
        Err(Unhashable)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<ArrayHash, Unhashable>, Unhashable> {
        Err(Unhashable)
    }
}

impl ser::SerializeSeq for ArraySerializer {
    type Ok = ArrayHash;
    type Error = Unhashable;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Unhashable> {
        self.push(value)
    }

    fn end(self) -> Result<ArrayHash, Unhashable> {
        Ok(self.array)
    }
}

impl ser::SerializeTuple for ArraySerializer {
    type Ok = ArrayHash;
    type Error = Unhashable;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Unhashable> {
        self.push(value)
    }

    fn end(self) -> Result<ArrayHash, Unhashable> {
        Ok(self.array)
    }
}

impl ser::SerializeTupleStruct for ArraySerializer {
    type Ok = ArrayHash;
    type Error = Unhashable;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Unhashable> {
        self.push(value)
    }

    fn end(self) -> Result<ArrayHash, Unhashable> {
        Ok(self.array)
    }
}
