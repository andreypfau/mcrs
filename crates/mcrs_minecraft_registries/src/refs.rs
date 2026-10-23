use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use derive_where::derive_where;
use mcrs_minecraft_core::{Id, Registered, ResourceKey, ResourceLocation, TagId, TagKey};
use mcrs_minecraft_datapack::JsonError;
use serde::de::value::{
    BoolDeserializer, F64Deserializer, I64Deserializer, MapAccessDeserializer,
    SeqAccessDeserializer, U64Deserializer,
};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};

use crate::static_key::StaticEntry;
use crate::tags::IdSet;

pub trait Inline: Clone + fmt::Debug + PartialEq + Send + Sync + 'static {}

impl<T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static> Inline for T {}

/// The form a value holds its references in: [`Named`] is the form of a data pack file and of
/// the registry data packet, [`Resolved`] the numbered form a loaded registry holds.
pub trait Refs: Inline + Copy + Eq {
    type Id<R: Registered>: Inline;
    type Set<R: Registered>: Inline;

    fn id_of<R: StaticEntry>(entry: R) -> Self::Id<R>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Named;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Resolved;

impl Refs for Named {
    type Id<R: Registered> = ResourceKey<R>;
    type Set<R: Registered> = KeySet<R>;

    fn id_of<R: StaticEntry>(entry: R) -> ResourceKey<R> {
        ResourceKey::from_location(entry.location().to_arc())
    }
}

impl Refs for Resolved {
    type Id<R: Registered> = Id<R>;
    type Set<R: Registered> = IdSet<R>;

    fn id_of<R: StaticEntry>(entry: R) -> Id<R> {
        entry.id()
    }
}

#[derive_where(Clone, Default, PartialEq, Eq, Hash)]
pub enum KeySet<R> {
    Tag(TagKey<R>),
    #[derive_where(default)]
    List(Box<[ResourceKey<R>]>),
}

impl<R> fmt::Debug for KeySet<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeySet::Tag(tag) => write!(f, "#{}", tag.as_str()),
            KeySet::List(keys) => f.debug_list().entries(keys.iter()).finish(),
        }
    }
}

impl<R> Serialize for KeySet<R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            KeySet::Tag(tag) => serializer.collect_str(&format_args!("#{}", tag.as_str())),
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

fn location<E: de::Error>(text: &str) -> Result<ResourceLocation, E> {
    ResourceLocation::read(text).map_err(E::custom)
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
                    Some(tag) => location(tag).map(|tag| KeySet::Tag(TagKey::from_location(tag))),
                    None => location(text)
                        .map(|key| KeySet::List(Box::new([ResourceKey::from_location(key)]))),
                }
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<KeySet<R>, A::Error> {
                let keys = Vec::<ResourceKey<R>>::deserialize(SeqAccessDeserializer::new(seq))?;
                Ok(KeySet::List(keys.into_boxed_slice()))
            }
        }

        deserializer.deserialize_any(SetVisitor(PhantomData))
    }
}

/// An entry of the registry `R`, or a value of it written in place. The value carries no bound
/// here: a value that holds text can hold an item that holds the value again.
#[derive_where(Clone)]
#[derive_where(PartialEq, Debug; V)]
pub enum Ref<M: Refs, R: Registered, V> {
    Id(M::Id<R>),
    Inline(Arc<V>),
}

impl<M: Refs, R: StaticEntry, V> Ref<M, R, V> {
    pub fn of(entry: R) -> Self {
        Ref::Id(M::id_of(entry))
    }
}

impl<R: Registered, V: Serialize> Serialize for Ref<Named, R, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Ref::Id(key) => key.serialize(serializer),
            Ref::Inline(value) => value.serialize(serializer),
        }
    }
}

// A string is always a key, never an inline value: an unknown key fails rather than becoming a
// value of its own.
impl<'de, R: Registered, V: Deserialize<'de>> Deserialize<'de> for Ref<Named, R, V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RefVisitor<R, V>(PhantomData<fn() -> (R, V)>);

        impl<'de, R: Registered, V: Deserialize<'de>> RefVisitor<R, V> {
            fn inline<D: Deserializer<'de>>(deserializer: D) -> Result<Ref<Named, R, V>, D::Error> {
                V::deserialize(deserializer).map(|value| Ref::Inline(Arc::new(value)))
            }
        }

        impl<'de, R: Registered, V: Deserialize<'de>> Visitor<'de> for RefVisitor<R, V> {
            type Value = Ref<Named, R, V>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an identifier or an inline value")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
                location(text).map(|key| Ref::Id(ResourceKey::from_location(key)))
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Self::inline(BoolDeserializer::new(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Self::inline(U64Deserializer::new(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Self::inline(I64Deserializer::new(v))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Self::inline(F64Deserializer::new(v))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Self::inline(SeqAccessDeserializer::new(seq))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Self::inline(MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(RefVisitor(PhantomData))
    }
}

pub trait Resolver {
    fn id<R: Registered>(&self, key: &ResourceKey<R>) -> Result<Id<R>, ResolveError>;

    fn tag<R: Registered>(&self, tag: &TagKey<R>) -> Result<TagId<R>, ResolveError>;
}

pub trait Namer {
    fn name<R: Registered>(&self, id: Id<R>) -> Option<&ResourceLocation>;

    fn tag_name<R: Registered>(&self, tag: TagId<R>) -> Option<&ResourceLocation>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    path: String,
    message: String,
    missing: Option<Box<MissingRef>>,
}

/// A name the registry it refers to does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingRef {
    pub(crate) registry: ResourceLocation,
    pub(crate) name: ResourceLocation,
    pub(crate) tag: bool,
}

impl ResolveError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            path: String::new(),
            message: message.into(),
            missing: None,
        }
    }

    pub(crate) fn missing(missing: MissingRef, message: String) -> Self {
        Self {
            missing: Some(Box::new(missing)),
            ..Self::new(message)
        }
    }

    pub(crate) fn json(error: JsonError) -> Self {
        Self {
            path: error.field.unwrap_or_default(),
            ..Self::new(error.message)
        }
    }

    pub(crate) fn missing_ref(&self) -> Option<&MissingRef> {
        self.missing.as_deref()
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

    pub fn path(&self) -> Option<&str> {
        (!self.path.is_empty()).then_some(self.path.as_str())
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

impl From<String> for ResolveError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

pub trait Resolve: Sized {
    type Named;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError>;

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError>;
}

fn unnamed<R>(id: Id<R>) -> ResolveError {
    ResolveError::new(format!("no entry is numbered {}", id.number()))
}

impl<R: Registered> Resolve for Id<R> {
    type Named = ResourceKey<R>;

    fn resolve<C: Resolver>(named: ResourceKey<R>, ctx: &C) -> Result<Self, ResolveError> {
        ctx.id(&named)
    }

    fn name<C: Namer>(&self, names: &C) -> Result<ResourceKey<R>, ResolveError> {
        names
            .name(*self)
            .map(|name| ResourceKey::from_location(name.clone()))
            .ok_or_else(|| unnamed(*self))
    }
}

impl<R: Registered> Resolve for IdSet<R> {
    type Named = KeySet<R>;

    fn resolve<C: Resolver>(named: KeySet<R>, ctx: &C) -> Result<Self, ResolveError> {
        match named {
            KeySet::Tag(tag) => ctx.tag(&tag).map(IdSet::Tag),
            KeySet::List(keys) => keys
                .iter()
                .enumerate()
                .map(|(at, key)| ctx.id(key).map_err(|e| e.at_index(at)))
                .collect::<Result<_, _>>()
                .map(IdSet::List),
        }
    }

    fn name<C: Namer>(&self, names: &C) -> Result<KeySet<R>, ResolveError> {
        match self {
            IdSet::Tag(tag) => names
                .tag_name(*tag)
                .map(|name| KeySet::Tag(TagKey::from_location(name.clone())))
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

impl<R: Registered, V: Resolve<Named: Clone>> Resolve for Ref<Resolved, R, V> {
    type Named = Ref<Named, R, V::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        match named {
            Ref::Id(key) => Id::resolve(key, ctx).map(Ref::Id),
            Ref::Inline(value) => V::resolve(Arc::unwrap_or_clone(value), ctx)
                .map(|value| Ref::Inline(Arc::new(value))),
        }
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        match self {
            Ref::Id(id) => id.name(names).map(Ref::Id),
            Ref::Inline(value) => value.name(names).map(|value| Ref::Inline(Arc::new(value))),
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

impl<K: Ord + Clone + fmt::Display, V: Resolve> Resolve for std::collections::BTreeMap<K, V> {
    type Named = std::collections::BTreeMap<K, V::Named>;

    fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
        named
            .into_iter()
            .map(|(key, value)| match V::resolve(value, ctx) {
                Ok(value) => Ok((key, value)),
                Err(error) => Err(error.at(&key)),
            })
            .collect()
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
        self.iter()
            .map(|(key, value)| Ok((key.clone(), value.name(names).map_err(|e| e.at(key))?)))
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
