use core::error::Error;
use core::fmt;

use serde::Serialize;
use serde::ser;
use serde::ser::Impossible;

use crate::BYTE_ARRAY_ID;
use crate::BYTE_ID;
use crate::COMPOUND_ID;
use crate::DOUBLE_ID;
use crate::END_ID;
use crate::FLOAT_ID;
use crate::INT_ID;
use crate::LIST_ID;
use crate::LONG_ID;
use crate::NBT_ARRAY_TAG;
use crate::SHORT_ID;
use crate::STRING_ID;

#[must_use]
pub fn depth_of<T: Serialize + ?Sized>(value: &T, budget: u32) -> Option<u32> {
    match value.serialize(DepthSerializer { budget }) {
        Ok(shape) => Some(shape.levels),
        Err(DepthFault::TooDeep) => Some(budget.saturating_add(1)),
        Err(DepthFault::NoTag) => None,
    }
}

#[derive(Debug)]
enum DepthFault {
    NoTag,
    TooDeep,
}

impl fmt::Display for DepthFault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTag => formatter.write_str("the value has no NBT tag"),
            Self::TooDeep => formatter.write_str("the value is nested too deep"),
        }
    }
}

impl Error for DepthFault {}

impl ser::Error for DepthFault {
    fn custom<T: fmt::Display>(_message: T) -> Self {
        Self::NoTag
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Absent,
    EmptyString,
    Wrapper,
    Array(&'static str),
    Tag(u8),
}

impl Kind {
    fn element_type(self) -> Self {
        match self {
            Self::Absent => Self::Tag(END_ID),
            Self::EmptyString => Self::Tag(STRING_ID),
            Self::Wrapper => Self::Tag(COMPOUND_ID),
            Self::Array(_) | Self::Tag(_) => self,
        }
    }
}

#[derive(Clone, Copy)]
struct Shape {
    levels: u32,
    kind: Kind,
}

impl Shape {
    fn leaf(kind: Kind) -> Result<Self, DepthFault> {
        Ok(Self { levels: 0, kind })
    }
}

struct DepthSerializer {
    budget: u32,
}

impl DepthSerializer {
    fn inner(self) -> Result<u32, DepthFault> {
        self.budget.checked_sub(1).ok_or(DepthFault::TooDeep)
    }
}

impl ser::Serializer for DepthSerializer {
    type Ok = Shape;
    type Error = DepthFault;
    type SerializeSeq = ListLevels;
    type SerializeTuple = ListLevels;
    type SerializeTupleStruct = ListLevels;
    type SerializeTupleVariant = Impossible<Shape, DepthFault>;
    type SerializeMap = CompoundLevels;
    type SerializeStruct = CompoundLevels;
    type SerializeStructVariant = Impossible<Shape, DepthFault>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, _: bool) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(BYTE_ID))
    }

    fn serialize_i8(self, _: i8) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(BYTE_ID))
    }

    fn serialize_i16(self, _: i16) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(SHORT_ID))
    }

    fn serialize_i32(self, _: i32) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(INT_ID))
    }

    fn serialize_i64(self, _: i64) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(LONG_ID))
    }

    fn serialize_u8(self, _: u8) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(BYTE_ID))
    }

    fn serialize_u16(self, _: u16) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(SHORT_ID))
    }

    fn serialize_u32(self, _: u32) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(INT_ID))
    }

    fn serialize_u64(self, _: u64) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(LONG_ID))
    }

    fn serialize_f32(self, _: f32) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(FLOAT_ID))
    }

    fn serialize_f64(self, _: f64) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(DOUBLE_ID))
    }

    fn serialize_char(self, _: char) -> Result<Shape, DepthFault> {
        Err(DepthFault::NoTag)
    }

    fn serialize_str(self, value: &str) -> Result<Shape, DepthFault> {
        if value.is_empty() {
            Shape::leaf(Kind::EmptyString)
        } else {
            Shape::leaf(Kind::Tag(STRING_ID))
        }
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Tag(BYTE_ARRAY_ID))
    }

    fn serialize_none(self) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Absent)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Shape, DepthFault> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Absent)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Shape, DepthFault> {
        Shape::leaf(Kind::Absent)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Shape, DepthFault> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Shape, DepthFault> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Shape, DepthFault> {
        if name == NBT_ARRAY_TAG {
            Shape::leaf(Kind::Array(variant))
        } else {
            Err(DepthFault::NoTag)
        }
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<ListLevels, DepthFault> {
        Ok(ListLevels::new(self.inner()?))
    }

    fn serialize_tuple(self, _len: usize) -> Result<ListLevels, DepthFault> {
        Ok(ListLevels::new(self.inner()?))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<ListLevels, DepthFault> {
        Ok(ListLevels::new(self.inner()?))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<Shape, DepthFault>, DepthFault> {
        Err(DepthFault::NoTag)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<CompoundLevels, DepthFault> {
        Ok(CompoundLevels::new(self.inner()?))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<CompoundLevels, DepthFault> {
        Ok(CompoundLevels::new(self.inner()?))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<Shape, DepthFault>, DepthFault> {
        Err(DepthFault::NoTag)
    }
}

struct ListLevels {
    budget: u32,
    element_type: Option<Kind>,
    plain: u32,
    wrapped: u32,
}

impl ListLevels {
    fn new(budget: u32) -> Self {
        Self {
            budget,
            element_type: None,
            plain: 0,
            wrapped: 0,
        }
    }

    fn finish(self) -> Shape {
        let deepest = if self.element_type == Some(Kind::Tag(COMPOUND_ID)) {
            self.wrapped
        } else {
            self.plain
        };
        Shape {
            levels: deepest.saturating_add(1),
            kind: Kind::Tag(LIST_ID),
        }
    }
}

impl ser::SerializeSeq for ListLevels {
    type Ok = Shape;
    type Error = DepthFault;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), DepthFault> {
        let shape = value.serialize(DepthSerializer {
            budget: self.budget,
        })?;
        let element_type = shape.kind.element_type();
        self.element_type = match self.element_type {
            Some(current) if current != element_type => Some(Kind::Tag(COMPOUND_ID)),
            Some(current) => Some(current),
            None => Some(element_type),
        };
        self.plain = self.plain.max(shape.levels);
        let wrapped = if shape.kind == Kind::Tag(COMPOUND_ID) {
            shape.levels
        } else {
            shape.levels.saturating_add(1)
        };
        self.wrapped = self.wrapped.max(wrapped);
        Ok(())
    }

    fn end(self) -> Result<Shape, DepthFault> {
        Ok(self.finish())
    }
}

impl ser::SerializeTuple for ListLevels {
    type Ok = Shape;
    type Error = DepthFault;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), DepthFault> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Shape, DepthFault> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for ListLevels {
    type Ok = Shape;
    type Error = DepthFault;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), DepthFault> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Shape, DepthFault> {
        ser::SerializeSeq::end(self)
    }
}

struct CompoundLevels {
    budget: u32,
    deepest: u32,
    entries: usize,
    empty_keys: usize,
    key: Kind,
}

impl CompoundLevels {
    fn new(budget: u32) -> Self {
        Self {
            budget,
            deepest: 0,
            entries: 0,
            empty_keys: 0,
            key: Kind::Absent,
        }
    }

    fn entry<T: Serialize + ?Sized>(&mut self, key: Kind, value: &T) -> Result<(), DepthFault> {
        let shape = value.serialize(DepthSerializer {
            budget: self.budget,
        })?;
        if shape.kind == Kind::Absent {
            return Ok(());
        }
        self.entries = self.entries.saturating_add(1);
        if key == Kind::EmptyString {
            self.empty_keys = self.empty_keys.saturating_add(1);
        }
        self.deepest = self.deepest.max(shape.levels);
        Ok(())
    }

    fn finish(self) -> Shape {
        let kind = if self.entries == 1 && self.empty_keys == 1 {
            Kind::Wrapper
        } else {
            Kind::Tag(COMPOUND_ID)
        };
        Shape {
            levels: self.deepest.saturating_add(1),
            kind,
        }
    }
}

impl ser::SerializeMap for CompoundLevels {
    type Ok = Shape;
    type Error = DepthFault;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), DepthFault> {
        self.key = key.serialize(DepthSerializer { budget: 0 })?.kind;
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), DepthFault> {
        self.entry(self.key, value)
    }

    fn end(self) -> Result<Shape, DepthFault> {
        Ok(self.finish())
    }
}

impl ser::SerializeStruct for CompoundLevels {
    type Ok = Shape;
    type Error = DepthFault;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), DepthFault> {
        let key = if key.is_empty() {
            Kind::EmptyString
        } else {
            Kind::Tag(STRING_ID)
        };
        self.entry(key, value)
    }

    fn end(self) -> Result<Shape, DepthFault> {
        Ok(self.finish())
    }
}
