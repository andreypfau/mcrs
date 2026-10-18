use crate::registry::Registry;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::resource_key::ResourceKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use serde::de::{self, Visitor};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::any::type_name;
use std::fmt;
use std::marker::PhantomData;

struct Named<R>(Id<R>);

struct NamedRef<'a, R>(&'a Id<R>);

impl<R: 'static> Serialize for NamedRef<'_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize(self.0, serializer)
    }
}

impl<'de, R: 'static> Deserialize<'de> for Named<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize(deserializer).map(Named)
    }
}

pub fn serialize<R: 'static, S: Serializer>(id: &Id<R>, serializer: S) -> Result<S::Ok, S::Error> {
    Registry::<R>::in_scope(type_name::<Id<R>>(), |registry| match registry.name(*id) {
        Some(name) => serializer.serialize_str(name.as_str()),
        None => Err(S::Error::custom(format_args!(
            "registry {} holds no entry numbered {}",
            registry.table().registry(),
            id.index()
        ))),
    })
    .unwrap_or_else(|error| Err(S::Error::custom(error)))
}

pub fn deserialize<'de, R: 'static, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Id<R>, D::Error> {
    struct IdVisitor<R>(PhantomData<fn() -> R>);

    impl<R: 'static> Visitor<'_> for IdVisitor<R> {
        type Value = Id<R>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "the name of an entry of registry {}",
                crate::set::label::<R>()
            )
        }

        fn visit_str<E: de::Error>(self, text: &str) -> Result<Id<R>, E> {
            let key =
                ResourceKey::<R>::from_location(ResourceLocation::read(text).map_err(E::custom)?);
            Registry::<R>::in_scope(type_name::<Id<R>>(), |registry| registry.require(&key))
                .map_err(E::custom)?
                .map_err(E::custom)
        }
    }

    deserializer.deserialize_str(IdVisitor(PhantomData))
}

pub mod option {
    use super::{Named, NamedRef};
    use mcrs_minecraft_core::Id;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<R: 'static, S: Serializer>(
        id: &Option<Id<R>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match id {
            Some(id) => serializer.serialize_some(&NamedRef(id)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, R: 'static, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Id<R>>, D::Error> {
        Option::<Named<R>>::deserialize(deserializer).map(|id| id.map(|Named(id)| id))
    }
}

pub mod vec {
    use super::{Named, NamedRef};
    use mcrs_minecraft_core::Id;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<R: 'static, S: Serializer>(
        ids: &[Id<R>],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(ids.iter().map(NamedRef))
    }

    pub fn deserialize<'de, R: 'static, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<Id<R>>, D::Error> {
        Vec::<Named<R>>::deserialize(deserializer)
            .map(|ids| ids.into_iter().map(|Named(id)| id).collect())
    }
}

pub mod array {
    use super::{Named, NamedRef};
    use mcrs_minecraft_core::Id;
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<R: 'static, S: Serializer, const N: usize>(
        ids: &[Id<R>; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(ids.iter().map(NamedRef))
    }

    pub fn deserialize<'de, R: 'static, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[Id<R>; N], D::Error> {
        let ids: Vec<Id<R>> = Vec::<Named<R>>::deserialize(deserializer)?
            .into_iter()
            .map(|Named(id)| id)
            .collect();
        let len = ids.len();
        ids.try_into()
            .map_err(|_| D::Error::invalid_length(len, &format!("an array of {N}").as_str()))
    }
}
