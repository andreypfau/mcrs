use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::{ResourceLocation, TagKeyBuf};

/// A borrowed reference to a tag of the registry `R`. It is `Copy`, and the generated
/// constants are `TagKey<'static, R>`; [`TagKeyBuf`] is the owned form.
pub struct TagKey<'a, R> {
    location: ResourceLocation<'a>,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for TagKey<'_, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R> Copy for TagKey<'_, R> {}

impl<'a, R> TagKey<'a, R> {
    pub const fn new(location: ResourceLocation<'a>) -> Self {
        TagKey {
            location,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub const fn location(self) -> ResourceLocation<'a> {
        self.location
    }

    /// The owned form, with one allocation.
    pub fn to_buf(self) -> TagKeyBuf<R> {
        TagKeyBuf::from_location(self.location.to_buf())
    }
}

impl<'a, R> From<&TagKey<'a, R>> for TagKey<'a, R> {
    fn from(key: &TagKey<'a, R>) -> Self {
        *key
    }
}

impl<R> PartialEq for TagKey<'_, R> {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl<R> Eq for TagKey<'_, R> {}

impl<R> PartialEq<TagKeyBuf<R>> for TagKey<'_, R> {
    fn eq(&self, other: &TagKeyBuf<R>) -> bool {
        *self == other.as_key()
    }
}

impl<R> Hash for TagKey<'_, R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

impl<R> fmt::Display for TagKey<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.location, f)
    }
}

impl<R> fmt::Debug for TagKey<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TagKey({})", self.location)
    }
}
