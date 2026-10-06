use crate::registry::Registry;
use mcrs_minecraft_core::resource_key::ResourceKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use serde::de::{self, Visitor};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::any::type_name;
use std::fmt;
use std::marker::PhantomData;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug)]
pub struct BlockStateId(pub u16);

impl From<u16> for BlockStateId {
    #[inline]
    fn from(id: u16) -> Self {
        BlockStateId(id)
    }
}

impl From<BlockStateId> for u16 {
    #[inline]
    fn from(id: BlockStateId) -> Self {
        id.0
    }
}

pub struct Id<R> {
    number: u16,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for Id<R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<R> Copy for Id<R> {}
impl<R> PartialEq for Id<R> {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number
    }
}
impl<R> Eq for Id<R> {}
impl<R> PartialOrd for Id<R> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<R> Ord for Id<R> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.number.cmp(&other.number)
    }
}
impl<R> std::hash::Hash for Id<R> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.number.hash(state);
    }
}
impl<R> std::fmt::Debug for Id<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({})", self.number)
    }
}

impl<R> Id<R> {
    pub(crate) const fn from_number(number: u16) -> Self {
        Id {
            number,
            _marker: PhantomData,
        }
    }

    #[doc(hidden)]
    pub const fn from_static_position(position: u16) -> Self {
        Self::from_number(position)
    }

    pub const fn index(self) -> usize {
        self.number as usize
    }

    pub const fn number(self) -> u16 {
        self.number
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NarrowError {
    pub registry: String,
    pub id: u16,
    pub bits: u32,
}

impl fmt::Display for NarrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id {} of registry {} does not fit in {} bits",
            self.id, self.registry, self.bits
        )
    }
}

impl std::error::Error for NarrowError {}

impl<R: 'static> Id<R> {
    pub fn narrow<N: TryFrom<u16>>(self) -> Result<N, NarrowError> {
        N::try_from(self.number).map_err(|_| NarrowError {
            registry: crate::set::label::<R>(),
            id: self.number,
            bits: (std::mem::size_of::<N>() * 8) as u32,
        })
    }
}

impl<R: 'static> Serialize for Id<R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Registry::<R>::in_scope(type_name::<Self>(), |registry| match registry.name(*self) {
            Some(name) => serializer.serialize_str(name.as_str()),
            None => Err(S::Error::custom(format_args!(
                "registry {} holds no entry numbered {}",
                registry.table().registry(),
                self.index()
            ))),
        })
        .unwrap_or_else(|error| Err(S::Error::custom(error)))
    }
}

impl<'de, R: 'static> Deserialize<'de> for Id<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
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
                let key = ResourceKey::<R>::from_location(
                    ResourceLocation::read(text).map_err(E::custom)?,
                );
                Registry::<R>::in_scope(type_name::<Id<R>>(), |registry| registry.require(&key))
                    .map_err(E::custom)?
                    .map_err(E::custom)
            }
        }

        deserializer.deserialize_str(IdVisitor(PhantomData))
    }
}

pub(crate) fn id_number(position: usize) -> Option<u16> {
    u16::try_from(position).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RegistryError;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::rl;
    use std::sync::Arc;

    struct Wide;

    impl Wide {
        const KEY: RegistryKey<Wide> = RegistryKey::new(rl!("minecraft:wide"));
    }

    fn names(len: usize) -> impl Iterator<Item = ResourceLocation<Arc<str>>> {
        (0..len).map(|n| ResourceLocation::<Arc<str>>::read(&format!("minecraft:n{n}")).unwrap())
    }

    fn registry_of(len: usize) -> Registry<Wide> {
        Registry::new(Wide::KEY, names(len)).unwrap()
    }

    fn id_at(registry: &Registry<Wide>, n: usize) -> Id<Wide> {
        registry.by_name(&format!("minecraft:n{n}")).unwrap()
    }

    #[test]
    fn an_id_narrows_to_eight_bits_up_to_255() {
        let registry = registry_of(257);
        assert_eq!(id_at(&registry, 255).narrow::<u8>(), Ok(255));
        assert_eq!(id_at(&registry, 0).narrow::<u8>(), Ok(0));
        let error = id_at(&registry, 256).narrow::<u8>().unwrap_err();
        assert_eq!(
            error,
            NarrowError {
                registry: type_name::<Wide>().to_owned(),
                id: 256,
                bits: 8
            }
        );
        let message = error.to_string();
        assert!(message.contains("tests::Wide"), "{message}");
        assert!(message.contains("256"), "{message}");
        assert!(message.contains("8 bits"), "{message}");
    }

    #[test]
    fn a_registry_numbers_ids_up_to_65535_and_refuses_more_entries() {
        let registry = registry_of(65536);
        assert_eq!(id_at(&registry, 65535).number(), u16::MAX);
        assert_eq!(registry.ids().count(), 65536);

        let error = Registry::<Wide>::new(Wide::KEY, names(65537)).unwrap_err();
        assert_eq!(
            error,
            RegistryError::TooManyEntries {
                registry: Wide::KEY.location().into(),
                len: 65537
            }
        );
        let message = error.to_string();
        assert!(message.contains("minecraft:wide"), "{message}");
        assert!(message.contains("65537"), "{message}");
    }
}
