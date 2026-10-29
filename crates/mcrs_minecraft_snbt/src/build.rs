use alloc::string::String;
use alloc::vec::Vec;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::error::IntegerType;
use crate::text::StrText;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Scalar {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl Scalar {
    /// `value` is already in the range of `ty`.
    pub(crate) fn integer(ty: IntegerType, value: i64) -> Self {
        match ty {
            IntegerType::Byte => Scalar::Byte(value as i8),
            IntegerType::Short => Scalar::Short(value as i16),
            IntegerType::Int => Scalar::Int(value as i32),
            IntegerType::Long => Scalar::Long(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArrayKind {
    Byte,
    Int,
    Long,
}

impl ArrayKind {
    pub(crate) const fn element(self) -> IntegerType {
        match self {
            ArrayKind::Byte => IntegerType::Byte,
            ArrayKind::Int => IntegerType::Int,
            ArrayKind::Long => IntegerType::Long,
        }
    }
}

/// What the read makes of the values it accepts. The read calls it only for parts it keeps.
pub(crate) trait Build {
    type Value;
    type Compound;
    type List;
    type Array;

    fn scalar(&mut self, scalar: Scalar) -> Self::Value;
    fn string(&mut self, text: StrText<'_>) -> Self::Value;
    fn compound(&mut self) -> Self::Compound;
    fn insert(&mut self, compound: &mut Self::Compound, key: StrText<'_>, value: Self::Value);
    fn finish_compound(&mut self, compound: Self::Compound) -> Self::Value;
    fn list(&mut self) -> Self::List;
    fn push(&mut self, list: &mut Self::List, value: Self::Value);
    fn finish_list(&mut self, list: Self::List) -> Self::Value;
    fn array(&mut self, kind: ArrayKind) -> Self::Array;
    /// `value` is already in the range of the element type of the array.
    fn push_element(&mut self, array: &mut Self::Array, value: i64);
    fn finish_array(&mut self, array: Self::Array) -> Self::Value;
}

/// Keeps nothing: the read only validates, and allocates nothing.
pub(crate) struct Validate;

impl Build for Validate {
    type Value = ();
    type Compound = ();
    type List = ();
    type Array = ();

    fn scalar(&mut self, _: Scalar) {}
    fn string(&mut self, _: StrText<'_>) {}
    fn compound(&mut self) {}
    fn insert(&mut self, _: &mut (), _: StrText<'_>, _: ()) {}
    fn finish_compound(&mut self, _: ()) {}
    fn list(&mut self) {}
    fn push(&mut self, _: &mut (), _: ()) {}
    fn finish_list(&mut self, _: ()) {}
    fn array(&mut self, _: ArrayKind) {}
    fn push_element(&mut self, _: &mut (), _: i64) {}
    fn finish_array(&mut self, _: ()) {}
}

pub(crate) struct Tags;

pub(crate) enum TagArray {
    Byte(Vec<u8>),
    Int(Vec<i32>),
    Long(Vec<i64>),
}

fn string(text: StrText<'_>) -> String {
    let mut out = String::with_capacity(text.len_hint());
    out.extend(text.chars());
    out
}

impl Build for Tags {
    type Value = NbtTag;
    type Compound = NbtCompound;
    type List = Vec<NbtTag>;
    type Array = TagArray;

    fn scalar(&mut self, scalar: Scalar) -> NbtTag {
        match scalar {
            Scalar::Byte(v) => NbtTag::Byte(v),
            Scalar::Short(v) => NbtTag::Short(v),
            Scalar::Int(v) => NbtTag::Int(v),
            Scalar::Long(v) => NbtTag::Long(v),
            Scalar::Float(v) => NbtTag::Float(v),
            Scalar::Double(v) => NbtTag::Double(v),
        }
    }

    fn string(&mut self, text: StrText<'_>) -> NbtTag {
        NbtTag::String(string(text))
    }

    fn compound(&mut self) -> NbtCompound {
        NbtCompound::new()
    }

    fn insert(&mut self, compound: &mut NbtCompound, key: StrText<'_>, value: NbtTag) {
        let key = string(key);
        match compound.child_tags.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => compound.child_tags.push((key, value)),
        }
    }

    fn finish_compound(&mut self, compound: NbtCompound) -> NbtTag {
        NbtTag::Compound(compound)
    }

    fn list(&mut self) -> Vec<NbtTag> {
        Vec::new()
    }

    fn push(&mut self, list: &mut Vec<NbtTag>, value: NbtTag) {
        list.push(value);
    }

    fn finish_list(&mut self, list: Vec<NbtTag>) -> NbtTag {
        NbtTag::List(list)
    }

    fn array(&mut self, kind: ArrayKind) -> TagArray {
        match kind {
            ArrayKind::Byte => TagArray::Byte(Vec::new()),
            ArrayKind::Int => TagArray::Int(Vec::new()),
            ArrayKind::Long => TagArray::Long(Vec::new()),
        }
    }

    fn push_element(&mut self, array: &mut TagArray, value: i64) {
        match array {
            TagArray::Byte(values) => values.push(value as u8),
            TagArray::Int(values) => values.push(value as i32),
            TagArray::Long(values) => values.push(value),
        }
    }

    fn finish_array(&mut self, array: TagArray) -> NbtTag {
        match array {
            TagArray::Byte(values) => NbtTag::ByteArray(values.into_boxed_slice()),
            TagArray::Int(values) => NbtTag::IntArray(values),
            TagArray::Long(values) => NbtTag::LongArray(values),
        }
    }
}
