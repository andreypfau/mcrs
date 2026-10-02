use crate::registry::Registry;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::registry_key::RegistryKey;
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

impl From<BlockStateId> for VoxelId {
    #[inline]
    fn from(id: BlockStateId) -> Self {
        VoxelId(id.0)
    }
}

impl From<VoxelId> for BlockStateId {
    #[inline]
    fn from(id: VoxelId) -> Self {
        BlockStateId(id.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug)]
pub struct ItemId(pub u16);

impl From<u16> for ItemId {
    #[inline]
    fn from(id: u16) -> Self {
        ItemId(id)
    }
}

impl From<ItemId> for u16 {
    #[inline]
    fn from(id: ItemId) -> Self {
        id.0
    }
}

pub struct Id<R> {
    number: u32,
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
    pub(crate) fn from_number(number: u32) -> Self {
        Id {
            number,
            _marker: PhantomData,
        }
    }

    pub fn index(self) -> usize {
        self.number as usize
    }
}

impl<R: RegistryKey> Serialize for Id<R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Registry::<R>::in_scope(type_name::<Self>(), |registry| match registry.key(*self) {
            Some(name) => serializer.serialize_str(name.as_str()),
            None => Err(S::Error::custom(format_args!(
                "registry {} holds no entry numbered {}",
                R::KEY,
                self.index()
            ))),
        })
        .unwrap_or_else(|error| Err(S::Error::custom(error)))
    }
}

impl<'de, R: RegistryKey> Deserialize<'de> for Id<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdVisitor<R>(PhantomData<fn() -> R>);

        impl<R: RegistryKey> Visitor<'_> for IdVisitor<R> {
            type Value = Id<R>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "the name of an entry of registry {}", R::KEY)
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Id<R>, E> {
                let name = ResourceLocation::read(text).map_err(E::custom)?;
                Registry::<R>::in_scope(type_name::<Id<R>>(), |registry| {
                    registry.require(name.as_str())
                })
                .map_err(E::custom)?
                .map_err(E::custom)
            }
        }

        deserializer.deserialize_str(IdVisitor(PhantomData))
    }
}

pub(crate) fn id_number(position: usize) -> Option<u32> {
    u32::try_from(position).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn a_position_past_the_ids_range_has_no_number() {
        assert_eq!(id_number(0), Some(0));
        assert_eq!(id_number(u32::MAX as usize), Some(u32::MAX));
        assert_eq!(id_number(u32::MAX as usize + 1), None);
    }

    #[test]
    fn ids_order_by_position() {
        let first = Id::<()>::from_number(0);
        let second = Id::<()>::from_number(1);
        assert!(second > first);
        assert!(first < second);
        assert_eq!(first.cmp(&first), std::cmp::Ordering::Equal);
        assert_eq!(first.max(second), second);
    }
}
