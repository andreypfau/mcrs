use crate::resource_location::ResourceLocation;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

/// Marker trait for types that have an associated Minecraft registry path segment.
///
/// Implement this on your registry element type (e.g. `Block`, `Item`) to enable
/// `TagKey<T>` path derivation.
pub trait TaggedRegistry {
    /// The path segment used in tag asset paths.
    ///
    /// e.g. `"block"` → tag files live at `namespace/tags/block/…`
    const REGISTRY_PATH: &'static str;
}

/// A typed reference to a tag in a specific registry.
///
/// Generic over storage `S`:
/// - `TagKey<T>` = `TagKey<T, &'static str>` — `Copy`, zero-alloc, const-constructible.
/// - `TagKey<T, Arc<str>>` — heap-allocated, for runtime-parsed tag references.
///
/// Cross-variant equality and hashing compare by string content (like `ResourceLocation`).
pub struct TagKey<T: TaggedRegistry, S = &'static str> {
    rl: ResourceLocation<S>,
    _marker: PhantomData<fn() -> T>,
}

// ── Clone / Copy ──

impl<T: TaggedRegistry, S: Clone> Clone for TagKey<T, S> {
    fn clone(&self) -> Self {
        TagKey {
            rl: self.rl.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: TaggedRegistry> Copy for TagKey<T, &'static str> {}

// ── Eq / Hash (cross-variant, by string content) ──

impl<T: TaggedRegistry, S: AsRef<str>, U: AsRef<str>> PartialEq<TagKey<T, U>> for TagKey<T, S> {
    fn eq(&self, other: &TagKey<T, U>) -> bool {
        self.rl.as_str() == other.rl.as_str()
    }
}

impl<T: TaggedRegistry, S: AsRef<str>> Eq for TagKey<T, S> {}

impl<T: TaggedRegistry, S: AsRef<str>> Hash for TagKey<T, S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rl.as_str().hash(state);
    }
}

// ── Static variant (`&'static str`) ──

impl<T: TaggedRegistry> TagKey<T, &'static str> {
    /// Create a tag key from a compile-time validated `ResourceLocation<&'static str>`.
    ///
    /// ```rust,ignore
    /// use mcrs_core::{rl, TagKey};
    /// const MY_TAG: TagKey<Block> = TagKey::new(rl!("minecraft:mineable/pickaxe"));
    /// ```
    pub const fn new(rl: ResourceLocation<&'static str>) -> Self {
        TagKey {
            rl,
            _marker: PhantomData,
        }
    }

    /// The `ResourceLocation` of the tag itself. Zero-alloc, `Copy`.
    #[inline]
    pub fn resource_location(&self) -> ResourceLocation<&'static str> {
        self.rl
    }

    /// The `ResourceLocation` of the tag, converted to the Arc variant.
    /// Use this when you need an owned key for HashMap insertion.
    pub fn resource_location_arc(&self) -> ResourceLocation<Arc<str>> {
        self.rl.to_arc()
    }
}

// ── Arc variant (runtime-parsed) ──

impl<T: TaggedRegistry> TagKey<T, Arc<str>> {
    /// Create a tag key from a runtime-parsed `ResourceLocation<Arc<str>>`.
    pub fn from_location(rl: ResourceLocation<Arc<str>>) -> Self {
        TagKey {
            rl,
            _marker: PhantomData,
        }
    }
}

// ── Generic accessors (any S: AsRef<str>) ──

impl<T: TaggedRegistry, S: AsRef<str>> TagKey<T, S> {
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

    /// The Bevy asset path for this tag's JSON file.
    ///
    /// Format: `{namespace}/tags/{REGISTRY_PATH}/{path}.json`
    pub fn asset_path(&self) -> String {
        format!(
            "{}/tags/{}/{}.json",
            self.rl.namespace(),
            T::REGISTRY_PATH,
            self.rl.path()
        )
    }

    /// Convert to the `Arc<str>` variant (heap-allocates if not already `Arc`).
    pub fn to_arc(&self) -> TagKey<T, Arc<str>> {
        TagKey {
            rl: self.rl.to_arc(),
            _marker: PhantomData,
        }
    }
}

// ── From static → Arc ──

impl<T: TaggedRegistry> From<TagKey<T, &'static str>> for TagKey<T, Arc<str>> {
    fn from(key: TagKey<T, &'static str>) -> Self {
        key.to_arc()
    }
}

// ── Debug ──

impl<T: TaggedRegistry, S: AsRef<str>> std::fmt::Debug for TagKey<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TagKey({})", self.rl.as_str())
    }
}
