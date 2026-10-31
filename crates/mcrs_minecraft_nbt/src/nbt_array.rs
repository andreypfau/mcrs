use serde::de;

// TODO: This is a bit hacky
pub const NBT_ARRAY_TAG: &str = "__nbt_array";
pub(crate) const NBT_INT_ARRAY_TAG: &str = "__nbt_int_array";
pub(crate) const NBT_LONG_ARRAY_TAG: &str = "__nbt_long_array";
pub(crate) const NBT_BYTE_ARRAY_TAG: &str = "__nbt_byte_array";

macro_rules! impl_array {
    ($name:ident, $variant:expr) => {
        pub fn $name<T: serde::Serialize, S: serde::Serializer>(
            input: T,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            serializer.serialize_newtype_variant(NBT_ARRAY_TAG, 0, $variant, &input)
        }
    };
}

impl_array!(nbt_int_array, NBT_INT_ARRAY_TAG);
impl_array!(nbt_long_array, NBT_LONG_ARRAY_TAG);
impl_array!(nbt_byte_array, NBT_BYTE_ARRAY_TAG);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(variant_identifier)]
pub enum ArrayKind {
    #[serde(rename = "__nbt_byte_array")]
    Byte,
    #[serde(rename = "__nbt_int_array")]
    Int,
    #[serde(rename = "__nbt_long_array")]
    Long,
}

impl ArrayKind {
    /// The variant name a deserializer presents the array under.
    pub const fn variant(self) -> &'static str {
        match self {
            ArrayKind::Byte => NBT_BYTE_ARRAY_TAG,
            ArrayKind::Int => NBT_INT_ARRAY_TAG,
            ArrayKind::Long => NBT_LONG_ARRAY_TAG,
        }
    }

    pub const fn width(self) -> usize {
        match self {
            ArrayKind::Byte => 1,
            ArrayKind::Int => 4,
            ArrayKind::Long => 8,
        }
    }
}

pub trait ArrayVisitor<'de>: Sized {
    type Value;

    /// `payload` is the array's elements as stored: big-endian, end to end.
    fn visit_array<E: de::Error>(self, kind: ArrayKind, payload: &[u8]) -> Result<Self::Value, E>;

    /// The value is no array and nothing of it has been read yet.
    fn visit_other<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error>;
}

/// Takes an array whole, as `deserialize_bytes` does, and says which of the
/// three arrays it was: the bytes alone do not carry their element width.
pub fn nbt_array<'de, D, V>(deserializer: D, visitor: V) -> Result<V::Value, D::Error>
where
    D: serde::Deserializer<'de>,
    V: ArrayVisitor<'de>,
{
    deserializer.deserialize_newtype_struct(NBT_ARRAY_TAG, Tagged(visitor))
}

struct Tagged<V>(V);

impl<'de, V: ArrayVisitor<'de>> de::Visitor<'de> for Tagged<V> {
    type Value = V::Value;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an NBT value")
    }

    fn visit_newtype_struct<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<V::Value, D::Error> {
        self.0.visit_other(deserializer)
    }

    fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<V::Value, A::Error> {
        let (kind, payload) = data.variant()?;
        de::VariantAccess::newtype_variant_seed(
            payload,
            Payload {
                kind,
                visitor: self.0,
            },
        )
    }
}

struct Payload<V> {
    kind: ArrayKind,
    visitor: V,
}

impl<'de, V: ArrayVisitor<'de>> de::DeserializeSeed<'de> for Payload<V> {
    type Value = V::Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, payload: D) -> Result<V::Value, D::Error> {
        payload.deserialize_bytes(self)
    }
}

impl<'de, V: ArrayVisitor<'de>> de::Visitor<'de> for Payload<V> {
    type Value = V::Value;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("the payload of an NBT array")
    }

    fn visit_bytes<E: de::Error>(self, payload: &[u8]) -> Result<V::Value, E> {
        self.visitor.visit_array(self.kind, payload)
    }
}
