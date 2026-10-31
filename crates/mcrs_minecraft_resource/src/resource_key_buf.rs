use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ResourceKey, ResourceLocation, ResourceLocationBuf};

/// An owned resource key: a [`ResourceLocationBuf`] typed by the registry `T`.
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct ResourceKeyBuf<T> {
    location: ResourceLocationBuf,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for ResourceKeyBuf<T> {
    fn clone(&self) -> Self {
        Self::from_location(self.location.clone())
    }
}

impl<T> ResourceKeyBuf<T> {
    pub fn from_location(location: ResourceLocationBuf) -> Self {
        ResourceKeyBuf {
            location,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn location(&self) -> &ResourceLocationBuf {
        &self.location
    }

    /// The borrowed form.
    #[inline]
    pub fn as_key(&self) -> ResourceKey<'_, T> {
        ResourceKey::new(self.location.as_location())
    }

    /// The canonical `namespace:path` text.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.location.as_str()
    }

    #[inline]
    pub fn namespace(&self) -> &str {
        self.location.namespace()
    }

    #[inline]
    pub fn path(&self) -> &str {
        self.location.path()
    }
}

impl<T> From<ResourceKey<'_, T>> for ResourceKeyBuf<T> {
    fn from(key: ResourceKey<'_, T>) -> Self {
        key.to_buf()
    }
}

impl<'a, T> From<&'a ResourceKeyBuf<T>> for ResourceKey<'a, T> {
    fn from(key: &'a ResourceKeyBuf<T>) -> Self {
        key.as_key()
    }
}

impl<T> equivalent::Equivalent<ResourceKeyBuf<T>> for ResourceKey<'_, T> {
    fn equivalent(&self, key: &ResourceKeyBuf<T>) -> bool {
        *self == key.as_key()
    }
}

impl<T> PartialEq for ResourceKeyBuf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl<T> Eq for ResourceKeyBuf<T> {}

impl<T> PartialEq<ResourceKey<'_, T>> for ResourceKeyBuf<T> {
    fn eq(&self, other: &ResourceKey<'_, T>) -> bool {
        self.as_key() == *other
    }
}

impl<T> PartialEq<ResourceLocation<'_>> for ResourceKeyBuf<T> {
    fn eq(&self, other: &ResourceLocation<'_>) -> bool {
        self.location == *other
    }
}

impl<T> Hash for ResourceKeyBuf<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

impl<T> PartialOrd for ResourceKeyBuf<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for ResourceKeyBuf<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.location.cmp(&other.location)
    }
}

impl<T> fmt::Display for ResourceKeyBuf<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.location, f)
    }
}

impl<T> fmt::Debug for ResourceKeyBuf<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.as_key(), f)
    }
}

impl<T> Serialize for ResourceKeyBuf<T> {
    fn serialize<Ser: Serializer>(&self, s: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.location.serialize(s)
    }
}

impl<'de, T> Deserialize<'de> for ResourceKeyBuf<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        ResourceLocationBuf::deserialize(d).map(Self::from_location)
    }
}
