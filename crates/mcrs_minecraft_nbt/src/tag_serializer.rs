use crate::Error;
use crate::compound::NbtCompound;
use crate::tag::NbtTag;
use serde::ser::Impossible;
use serde::{Serialize, ser};

pub fn to_nbt_compound<T: Serialize>(value: &T) -> Result<NbtCompound, Error> {
    let tag = value.serialize(TagSerializer)?;
    match tag {
        NbtTag::Compound(c) => Ok(c),
        other => Err(Error::SerdeError(format!(
            "Root must be a compound, got {other:?}"
        ))),
    }
}

pub struct TagSerializer;

pub struct ListSerializer {
    elements: Vec<NbtTag>,
}

pub struct MapSerializer {
    entries: Vec<(String, NbtTag)>,
    next_key: Option<String>,
}

pub struct StructSerializer {
    entries: Vec<(String, NbtTag)>,
}

impl serde::ser::Serializer for TagSerializer {
    type Ok = NbtTag;
    type Error = Error;
    type SerializeSeq = ListSerializer;
    type SerializeTuple = ListSerializer;
    type SerializeTupleStruct = ListSerializer;
    type SerializeTupleVariant = Impossible<NbtTag, Self::Error>;
    type SerializeMap = MapSerializer;
    type SerializeStruct = StructSerializer;
    type SerializeStructVariant = Impossible<NbtTag, Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Byte(if v { 1 } else { 0 }))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Byte(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Short(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Int(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Long(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Byte(v as i8))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Short(v as i16))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Int(v as i32))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Long(v as i64))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Float(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::Double(v))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedType("char".to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::String(v.to_string()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::ByteArray(Box::from(v.to_vec())))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::End)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(TagSerializer)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::End)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::End)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::String(variant.to_string()))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(TagSerializer)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::UnsupportedType(format!(
            "newtype variant {name}::{variant} in in-memory serializer is not implemented"
        )))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ListSerializer {
            elements: Vec::new(),
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(ListSerializer {
            elements: Vec::new(),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(ListSerializer {
            elements: Vec::new(),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::UnsupportedType("tuple variant".to_string()))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(MapSerializer {
            entries: Vec::new(),
            next_key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(StructSerializer {
            entries: Vec::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::UnsupportedType("struct variant".to_string()))
    }
}

impl serde::ser::SerializeSeq for ListSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_element<T: ?Sized + serde::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        let tag = value.serialize(TagSerializer)?;
        // можно добавить проверку типа для однородности списка, если нужно
        self.elements.push(tag);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(NbtTag::List(self.elements))
    }
}

impl ser::SerializeTuple for ListSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_element<T: ?Sized + serde::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for ListSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_field<T: ?Sized + serde::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeStruct for StructSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_field<T: ?Sized + serde::Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        let tag = value.serialize(TagSerializer)?;
        if !matches!(tag, NbtTag::End) {
            self.entries.push((key.to_string(), tag));
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let compound: NbtCompound = self.entries.into_iter().collect();
        Ok(NbtTag::Compound(compound))
    }
}

impl ser::SerializeMap for MapSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_key<T: ?Sized + serde::Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        let tag = key.serialize(TagSerializer)?;
        let s = match tag {
            NbtTag::String(s) => s,
            _ => {
                return Err(Error::SerdeError(
                    "Map key must be a string in NBT".to_string(),
                ));
            }
        };
        self.next_key = Some(s);
        Ok(())
    }

    fn serialize_value<T: ?Sized + serde::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Self::Error> {
        let key = self
            .next_key
            .take()
            .ok_or_else(|| Error::SerdeError("serialize_value without key".to_string()))?;

        let tag = value.serialize(TagSerializer)?;
        if !matches!(tag, NbtTag::End) {
            self.entries.push((key, tag));
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let compound: NbtCompound = self.entries.into_iter().collect();
        Ok(NbtTag::Compound(compound))
    }
}
