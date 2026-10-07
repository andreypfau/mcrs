use crate::resource_location::ResourceLocation;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

/// A typed reference to a tag in a specific registry.
///
/// Generic over storage `S`:
/// - `TagKey<R>` = `TagKey<R, Arc<str>>` — heap-allocated, for runtime-parsed tag references.
/// - `TagKey<R, &'static str>` — `Copy`, zero-alloc, const-constructible; every constant spells it.
///
/// Cross-variant equality and hashing compare by string content (like `ResourceLocation`).
pub struct TagKey<R, S = Arc<str>> {
    rl: ResourceLocation<S>,
    _marker: PhantomData<fn() -> R>,
}

// ── Clone / Copy ──

impl<R, S: Clone> Clone for TagKey<R, S> {
    fn clone(&self) -> Self {
        TagKey {
            rl: self.rl.clone(),
            _marker: PhantomData,
        }
    }
}

impl<R> Copy for TagKey<R, &'static str> {}

// ── Eq / Hash (cross-variant, by string content) ──

impl<R, S: AsRef<str>, U: AsRef<str>> PartialEq<TagKey<R, U>> for TagKey<R, S> {
    fn eq(&self, other: &TagKey<R, U>) -> bool {
        self.rl.as_str() == other.rl.as_str()
    }
}

impl<R, S: AsRef<str>> Eq for TagKey<R, S> {}

impl<R, S: AsRef<str>> Hash for TagKey<R, S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rl.as_str().hash(state);
    }
}

// ── Static variant (`&'static str`) ──

impl<R> TagKey<R, &'static str> {
    /// Create a tag key from a compile-time validated `ResourceLocation<&'static str>`.
    ///
    /// ```rust,ignore
    /// use mcrs_minecraft_core::{rl, TagKey};
    /// const MY_TAG: TagKey<Block, &'static str> = TagKey::new(rl!("minecraft:mineable/pickaxe"));
    /// ```
    pub const fn new(rl: ResourceLocation<&'static str>) -> Self {
        TagKey {
            rl,
            _marker: PhantomData,
        }
    }
}

// ── Arc variant (runtime-parsed) ──

impl<R> TagKey<R, Arc<str>> {
    /// Create a tag key from a runtime-parsed `ResourceLocation<Arc<str>>`.
    pub fn from_location(rl: ResourceLocation<Arc<str>>) -> Self {
        TagKey {
            rl,
            _marker: PhantomData,
        }
    }
}

// ── Generic accessors (any S: AsRef<str>) ──

impl<R, S: AsRef<str>> TagKey<R, S> {
    /// The full `namespace:path` string of this tag key.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.rl.as_str()
    }

    /// Borrow the inner `ResourceLocation`.
    #[inline]
    pub fn location(&self) -> &ResourceLocation<S> {
        &self.rl
    }
}

// ── Debug ──

impl<R, S: AsRef<str>> std::fmt::Debug for TagKey<R, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TagKey({})", self.rl.as_str())
    }
}
