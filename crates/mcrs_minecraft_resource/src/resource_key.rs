use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use serde::{Serialize, Serializer};

use crate::{ResourceKeyBuf, ResourceLocation};

/// A borrowed location typed by the registry `T` it names an entry of. It is `Copy`, and the
/// generated constants are `ResourceKey<'static, T>`; [`ResourceKeyBuf`] is the owned form.
pub struct ResourceKey<'a, T> {
    location: ResourceLocation<'a>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for ResourceKey<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ResourceKey<'_, T> {}

impl<'a, T> ResourceKey<'a, T> {
    pub const fn new(location: ResourceLocation<'a>) -> Self {
        ResourceKey {
            location,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub const fn location(self) -> ResourceLocation<'a> {
        self.location
    }

    #[inline]
    pub const fn namespace(self) -> &'a str {
        self.location.namespace()
    }

    #[inline]
    pub const fn path(self) -> &'a str {
        self.location.path()
    }

    /// The owned form, with one allocation.
    pub fn to_buf(self) -> ResourceKeyBuf<T> {
        ResourceKeyBuf::from_location(self.location.to_buf())
    }
}

impl<'a, T> From<&ResourceKey<'a, T>> for ResourceKey<'a, T> {
    fn from(key: &ResourceKey<'a, T>) -> Self {
        *key
    }
}

impl<T> PartialEq for ResourceKey<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl<T> Eq for ResourceKey<'_, T> {}

impl<T> PartialEq<ResourceKeyBuf<T>> for ResourceKey<'_, T> {
    fn eq(&self, other: &ResourceKeyBuf<T>) -> bool {
        *self == other.as_key()
    }
}

impl<T> Hash for ResourceKey<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

impl<T> PartialOrd for ResourceKey<'_, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for ResourceKey<'_, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.location.cmp(&other.location)
    }
}

impl<T> fmt::Display for ResourceKey<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.location, f)
    }
}

impl<T> fmt::Debug for ResourceKey<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ResourceKey<{}>({})",
            core::any::type_name::<T>(),
            self.location
        )
    }
}

impl<T> Serialize for ResourceKey<'_, T> {
    fn serialize<Ser: Serializer>(&self, s: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.location.serialize(s)
    }
}
