use crate::resource_location::ResourceLocation;
use std::marker::PhantomData;
use std::sync::Arc;

/// Marker trait for types that have an associated Minecraft registry path segment.
///
/// Implement this on your registry element type (e.g. `Block`, `Item`) to enable
/// `TagKey<T>` path derivation.
pub trait TagRegistryType {
    /// The path segment used in tag asset paths.
    ///
    /// e.g. `"block"` → tag files live at `namespace/tags/block/…`
    const REGISTRY_PATH: &'static str;
}

/// A typed, const-compatible reference to a tag in a specific registry.
///
/// Stores a `ResourceLocation<&'static str>` — `Copy`, zero-alloc.
///
/// Prefer constructing with `TagKey::new(rl!("minecraft:mineable/pickaxe"))`.
pub struct TagKey<T: TagRegistryType> {
    rl: ResourceLocation<&'static str>,
    _marker: PhantomData<fn() -> T>,
}

// Manual impls so T doesn't need Clone/Copy.
impl<T: TagRegistryType> Clone for TagKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: TagRegistryType> Copy for TagKey<T> {}

impl<T: TagRegistryType> TagKey<T> {
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
}

impl<T: TagRegistryType> std::fmt::Debug for TagKey<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TagKey({})", self.rl)
    }
}
