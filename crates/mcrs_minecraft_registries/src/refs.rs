use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use mcrs_minecraft_core::{Id, ResourceLocation, TagId};
use serde::de::value::{
    BoolDeserializer, F64Deserializer, I64Deserializer, MapAccessDeserializer,
    SeqAccessDeserializer, U64Deserializer,
};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};

use crate::id::Key;
use crate::registry::RegistryType;
use crate::static_key::StaticKey;
use crate::tags::IdSet;

pub trait Inline: Clone + fmt::Debug + PartialEq + Send + Sync + 'static {}

impl<T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static> Inline for T {}

/// The form a value holds its references in: [`Named`] is the form of a data pack file and of
/// the registry data packet, [`Resolved`] the numbered form a loaded registry holds.
pub trait Refs: Inline + Copy + Eq {
    type Id<R: RegistryType>: Inline;
    type Set<R: RegistryType>: Inline;
    type Ref<R: RegistryType, V: Inline>: Inline;

    fn id_of<R: RegistryType>(key: StaticKey<R>) -> Self::Id<R>;

    fn ref_of<R: RegistryType, V: Inline>(key: StaticKey<R>) -> Self::Ref<R, V>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Named;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Resolved;

impl Refs for Named {
    type Id<R: RegistryType> = Key<R>;
    type Set<R: RegistryType> = KeySet<R>;
    type Ref<R: RegistryType, V: Inline> = KeyOr<R, V>;

    fn id_of<R: RegistryType>(key: StaticKey<R>) -> Key<R> {
        Key::new(key.location().to_arc())
    }

    fn ref_of<R: RegistryType, V: Inline>(key: StaticKey<R>) -> KeyOr<R, V> {
        KeyOr::Key(Self::id_of(key))
    }
}

impl Refs for Resolved {
    type Id<R: RegistryType> = Id<R>;
    type Set<R: RegistryType> = IdSet<R>;
    type Ref<R: RegistryType, V: Inline> = Ref<R, V>;

    fn id_of<R: RegistryType>(key: StaticKey<R>) -> Id<R> {
        key.id()
    }

    fn ref_of<R: RegistryType, V: Inline>(key: StaticKey<R>) -> Ref<R, V> {
        Ref::Id(key.id())
    }
}

pub enum KeySet<R> {
    Tag(Key<R>),
    List(Box<[Key<R>]>),
}

impl<R> Clone for KeySet<R> {
    fn clone(&self) -> Self {
        match self {
            KeySet::Tag(tag) => KeySet::Tag(tag.clone()),
            KeySet::List(keys) => KeySet::List(keys.clone()),
        }
    }
}

impl<R> PartialEq for KeySet<R> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (KeySet::Tag(a), KeySet::Tag(b)) => a == b,
            (KeySet::List(a), KeySet::List(b)) => a == b,
            _ => false,
        }
    }
}

impl<R> fmt::Debug for KeySet<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeySet::Tag(tag) => write!(f, "#{}", tag.location()),
            KeySet::List(keys) => f.debug_list().entries(keys.iter()).finish(),
        }
    }
}

impl<R> Serialize for KeySet<R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            KeySet::Tag(tag) => serializer.collect_str(&format_args!("#{}", tag.location())),
            KeySet::List(keys) => match &**keys {
                [key] => key.serialize(serializer),
                keys => {
                    let mut seq = serializer.serialize_seq(Some(keys.len()))?;
                    for key in keys {
                        seq.serialize_element(key)?;
                    }
                    seq.end()
                }
            },
        }
    }
}

fn key<R, E: de::Error>(text: &str) -> Result<Key<R>, E> {
    ResourceLocation::read(text)
        .map(Key::new)
        .map_err(E::custom)
}

impl<'de, R> Deserialize<'de> for KeySet<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor<R>(PhantomData<fn() -> R>);

        impl<'de, R> Visitor<'de> for SetVisitor<R> {
            type Value = KeySet<R>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a #tag, an identifier or a list of identifiers")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<KeySet<R>, E> {
                match text.strip_prefix('#') {
                    Some(tag) => key(tag).map(KeySet::Tag),
                    None => key(text).map(|key| KeySet::List(Box::new([key]))),
                }
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<KeySet<R>, A::Error> {
                let keys = Vec::<Key<R>>::deserialize(SeqAccessDeserializer::new(seq))?;
                Ok(KeySet::List(keys.into_boxed_slice()))
            }
        }

        deserializer.deserialize_any(SetVisitor(PhantomData))
    }
}

pub enum KeyOr<R, V> {
    Key(Key<R>),
    Inline(Box<V>),
}

impl<R, V: Clone> Clone for KeyOr<R, V> {
    fn clone(&self) -> Self {
        match self {
            KeyOr::Key(key) => KeyOr::Key(key.clone()),
            KeyOr::Inline(value) => KeyOr::Inline(value.clone()),
        }
    }
}

impl<R, V: PartialEq> PartialEq for KeyOr<R, V> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (KeyOr::Key(a), KeyOr::Key(b)) => a == b,
            (KeyOr::Inline(a), KeyOr::Inline(b)) => a == b,
            _ => false,
        }
    }
}

impl<R, V: fmt::Debug> fmt::Debug for KeyOr<R, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyOr::Key(key) => key.fmt(f),
            KeyOr::Inline(value) => f.debug_tuple("Inline").field(value).finish(),
        }
    }
}

impl<R, V: Serialize> Serialize for KeyOr<R, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            KeyOr::Key(key) => key.serialize(serializer),
            KeyOr::Inline(value) => value.serialize(serializer),
        }
    }
}

// A string is always a key, never an inline value: an unknown key fails rather than becoming a
// value of its own.
impl<'de, R, V: Deserialize<'de>> Deserialize<'de> for KeyOr<R, V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrVisitor<R, V>(PhantomData<fn() -> (R, V)>);

        impl<'de, R, V: Deserialize<'de>> OrVisitor<R, V> {
            fn inline<D: Deserializer<'de>>(deserializer: D) -> Result<KeyOr<R, V>, D::Error> {
                V::deserialize(deserializer).map(|value| KeyOr::Inline(Box::new(value)))
            }
        }

        impl<'de, R, V: Deserialize<'de>> Visitor<'de> for OrVisitor<R, V> {
            type Value = KeyOr<R, V>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an identifier or an inline value")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<KeyOr<R, V>, E> {
                key(text).map(KeyOr::Key)
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<KeyOr<R, V>, E> {
                Self::inline(BoolDeserializer::new(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<KeyOr<R, V>, E> {
                Self::inline(U64Deserializer::new(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<KeyOr<R, V>, E> {
                Self::inline(I64Deserializer::new(v))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<KeyOr<R, V>, E> {
                Self::inline(F64Deserializer::new(v))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<KeyOr<R, V>, A::Error> {
                Self::inline(SeqAccessDeserializer::new(seq))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<KeyOr<R, V>, A::Error> {
                Self::inline(MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(OrVisitor(PhantomData))
    }
}

pub enum Ref<R: RegistryType, V = <R as RegistryType>::Value> {
    Id(Id<R>),
    Inline(Arc<V>),
}

impl<R: RegistryType, V> Clone for Ref<R, V> {
    fn clone(&self) -> Self {
        match self {
            Ref::Id(id) => Ref::Id(*id),
            Ref::Inline(value) => Ref::Inline(Arc::clone(value)),
        }
    }
}

impl<R: RegistryType, V: PartialEq> PartialEq for Ref<R, V> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Ref::Id(a), Ref::Id(b)) => a == b,
            (Ref::Inline(a), Ref::Inline(b)) => a == b,
            _ => false,
        }
    }
}

impl<R: RegistryType, V: fmt::Debug> fmt::Debug for Ref<R, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ref::Id(id) => f.debug_tuple("Id").field(id).finish(),
            Ref::Inline(value) => f.debug_tuple("Inline").field(value).finish(),
        }
    }
}

pub trait Resolver {
    fn id<R: RegistryType>(&self, key: &Key<R>) -> Result<Id<R>, String>;

    fn tag<R: RegistryType>(&self, tag: &Key<R>) -> Result<TagId<R>, String>;
}

pub trait Namer {
    fn name<R: RegistryType>(&self, id: Id<R>) -> Option<&ResourceLocation>;

    fn tag_name<R: RegistryType>(&self, tag: TagId<R>) -> Option<&ResourceLocation>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    path: String,
    message: String,
}

impl ResolveError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            path: String::new(),
            message: message.into(),
        }
    }

    pub fn at(self, field: impl fmt::Display) -> Self {
        self.prefixed(field.to_string())
    }

    pub fn at_index(self, index: usize) -> Self {
        self.prefixed(format!("[{index}]"))
    }

    fn prefixed(mut self, segment: String) -> Self {
        self.path = match self.path.is_empty() || self.path.starts_with('[') {
            true => segment + &self.path,
            false => format!("{segment}.{}", self.path),
        };
        self
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.path.is_empty() {
            true => f.write_str(&self.message),
            false => write!(f, "{}: {}", self.path, self.message),
        }
    }
}

impl std::error::Error for ResolveError {}

pub trait Resolve: Sized {
    type Named;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError>;

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError>;
}

fn unnamed<R>(id: Id<R>) -> ResolveError {
    ResolveError::new(format!("no entry is numbered {}", id.number()))
}

impl<R: RegistryType> Resolve for Id<R> {
    type Named = Key<R>;

    fn resolve<C: Resolver>(named: Key<R>, ctx: &C) -> Result<Self, ResolveError> {
        ctx.id(&named).map_err(ResolveError::new)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Key<R>, ResolveError> {
        names
            .name(*self)
            .map(|name| Key::new(name.clone()))
            .ok_or_else(|| unnamed(*self))
    }
}

impl<R: RegistryType> Resolve for IdSet<R> {
    type Named = KeySet<R>;

    fn resolve<C: Resolver>(named: KeySet<R>, ctx: &C) -> Result<Self, ResolveError> {
        match named {
            KeySet::Tag(tag) => ctx.tag(&tag).map(IdSet::Tag).map_err(ResolveError::new),
            KeySet::List(keys) => keys
                .iter()
                .enumerate()
                .map(|(at, key)| ctx.id(key).map_err(|e| ResolveError::new(e).at_index(at)))
                .collect::<Result<_, _>>()
                .map(IdSet::List),
        }
    }

    fn name<C: Namer>(&self, names: &C) -> Result<KeySet<R>, ResolveError> {
        match self {
            IdSet::Tag(tag) => names
                .tag_name(*tag)
                .map(|name| KeySet::Tag(Key::new(name.clone())))
                .ok_or_else(|| ResolveError::new(format!("no tag is numbered {}", tag.number()))),
            IdSet::List(ids) => ids
                .iter()
                .enumerate()
                .map(|(at, id)| id.name(names).map_err(|e| e.at_index(at)))
                .collect::<Result<_, _>>()
                .map(KeySet::List),
        }
    }
}

impl<R: RegistryType, V: Resolve> Resolve for Ref<R, V> {
    type Named = KeyOr<R, V::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        match named {
            KeyOr::Key(key) => Id::resolve(key, ctx).map(Ref::Id),
            KeyOr::Inline(value) => {
                V::resolve(*value, ctx).map(|value| Ref::Inline(Arc::new(value)))
            }
        }
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        match self {
            Ref::Id(id) => id.name(names).map(KeyOr::Key),
            Ref::Inline(value) => value
                .name(names)
                .map(|value| KeyOr::Inline(Box::new(value))),
        }
    }
}

impl<T: Resolve> Resolve for Option<T> {
    type Named = Option<T::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        named.map(|value| T::resolve(value, ctx)).transpose()
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        self.as_ref().map(|value| value.name(names)).transpose()
    }
}

impl<T: Resolve> Resolve for Vec<T> {
    type Named = Vec<T::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        named
            .into_iter()
            .enumerate()
            .map(|(at, value)| T::resolve(value, ctx).map_err(|e| e.at_index(at)))
            .collect()
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        self.iter()
            .enumerate()
            .map(|(at, value)| value.name(names).map_err(|e| e.at_index(at)))
            .collect()
    }
}

impl<T: Resolve, const N: usize> Resolve for [T; N] {
    type Named = [T::Named; N];

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        let values: Vec<T> = Vec::resolve(named.into(), ctx)?;
        values
            .try_into()
            .map_err(|_| ResolveError::new(format!("expected {N} values")))
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        let values: Vec<T::Named> = self
            .iter()
            .enumerate()
            .map(|(at, value)| value.name(names).map_err(|e| e.at_index(at)))
            .collect::<Result<_, _>>()?;
        values
            .try_into()
            .map_err(|_| ResolveError::new(format!("expected {N} values")))
    }
}

impl<T: Resolve, const MAX: usize> Resolve for mcrs_minecraft_core::Bounded<T, MAX> {
    type Named = mcrs_minecraft_core::Bounded<T::Named, MAX>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        T::resolve(named.0, ctx).map(mcrs_minecraft_core::Bounded)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        self.0.name(names).map(mcrs_minecraft_core::Bounded)
    }
}

impl<T: Resolve> Resolve for mcrs_minecraft_core::codec::CompactList<T> {
    type Named = mcrs_minecraft_core::codec::CompactList<T::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        Vec::resolve(named.0, ctx).map(mcrs_minecraft_core::codec::CompactList)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        self.0
            .name(names)
            .map(mcrs_minecraft_core::codec::CompactList)
    }
}

impl<T: Resolve> Resolve for Box<T> {
    type Named = Box<T::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        T::resolve(*named, ctx).map(Box::new)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        (**self).name(names).map(Box::new)
    }
}

/// Implements [`Resolve`] for types that hold no reference, so that they can be the inline value
/// of a [`Ref`].
#[macro_export]
macro_rules! resolve_as_is {
    ($($value:ty),+ $(,)?) => {$(
        impl $crate::Resolve for $value {
            type Named = Self;

            fn resolve<C: $crate::Resolver>(
                named: Self,
                _: &C,
            ) -> ::core::result::Result<Self, $crate::ResolveError> {
                ::core::result::Result::Ok(named)
            }

            fn name<C: $crate::Namer>(
                &self,
                _: &C,
            ) -> ::core::result::Result<Self, $crate::ResolveError> {
                ::core::result::Result::Ok(::core::clone::Clone::clone(self))
            }
        }
    )+};
}
