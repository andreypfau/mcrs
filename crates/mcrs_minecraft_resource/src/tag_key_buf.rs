use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::{ResourceLocationBuf, TagKey};

/// An owned reference to a tag of the registry `R`, such as one read from a data pack.
pub struct TagKeyBuf<R> {
    location: ResourceLocationBuf,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for TagKeyBuf<R> {
    fn clone(&self) -> Self {
        Self::from_location(self.location.clone())
    }
}

impl<R> TagKeyBuf<R> {
    pub fn from_location(location: ResourceLocationBuf) -> Self {
        TagKeyBuf {
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
    pub fn as_key(&self) -> TagKey<'_, R> {
        TagKey::new(self.location.as_location())
    }

    /// The canonical `namespace:path` text.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.location.as_str()
    }
}

impl<R> From<TagKey<'_, R>> for TagKeyBuf<R> {
    fn from(key: TagKey<'_, R>) -> Self {
        key.to_buf()
    }
}

impl<'a, R> From<&'a TagKeyBuf<R>> for TagKey<'a, R> {
    fn from(key: &'a TagKeyBuf<R>) -> Self {
        key.as_key()
    }
}

impl<R> equivalent::Equivalent<TagKeyBuf<R>> for TagKey<'_, R> {
    fn equivalent(&self, key: &TagKeyBuf<R>) -> bool {
        *self == key.as_key()
    }
}

impl<R> PartialEq for TagKeyBuf<R> {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl<R> Eq for TagKeyBuf<R> {}

impl<R> PartialEq<TagKey<'_, R>> for TagKeyBuf<R> {
    fn eq(&self, other: &TagKey<'_, R>) -> bool {
        self.as_key() == *other
    }
}

impl<R> Hash for TagKeyBuf<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

impl<R> fmt::Display for TagKeyBuf<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.location, f)
    }
}

impl<R> fmt::Debug for TagKeyBuf<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.as_key(), f)
    }
}
