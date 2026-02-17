use crate::resource_location::ResourceLocation;
use std::marker::PhantomData;

/// Marker trait for types that have an associated Minecraft registry path segment.
///
/// Implement this on your registry element type (e.g. `Block`, `Item`) to enable
/// `TagKey<T>` path derivation.
///
/// Example:
/// ```rust
/// use mcrs_core::tag::key::TagRegistryType;
/// struct Block;
/// impl TagRegistryType for Block {
///     const REGISTRY_PATH: &'static str = "block";
/// }
/// ```
pub trait TagRegistryType {
    /// The path segment used in tag asset paths.
    ///
    /// e.g. `"block"` → tag files live at `namespace/tags/block/…`
    const REGISTRY_PATH: &'static str;
}

/// A typed, const-compatible reference to a tag in a specific registry.
///
/// `TagKey<T>` stores `&'static str` rather than `ResourceLocation` so it can
/// be constructed as a `const`.
///
/// ```rust
/// use mcrs_core::tag::key::{TagKey, TagRegistryType};
/// struct Block;
/// impl TagRegistryType for Block { const REGISTRY_PATH: &'static str = "block"; }
///
/// const MINEABLE_PICKAXE: TagKey<Block> = TagKey::of("minecraft", "mineable/pickaxe");
/// ```
pub struct TagKey<T: TagRegistryType> {
    pub namespace: &'static str,
    pub path: &'static str,
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
    pub const fn of(namespace: &'static str, path: &'static str) -> Self {
        TagKey {
            namespace,
            path,
            _marker: PhantomData,
        }
    }

    /// The `ResourceLocation` of the tag itself (without registry path prefix).
    pub fn resource_location(&self) -> ResourceLocation {
        ResourceLocation::new(self.namespace, self.path)
    }

    /// The Bevy asset path for this tag's JSON file.
    ///
    /// Format: `{namespace}/tags/{REGISTRY_PATH}/{path}.json`
    ///
    /// Example: `TagKey::<Block>::of("minecraft", "mineable/pickaxe")`
    /// → `"minecraft/tags/block/mineable/pickaxe.json"`
    pub fn asset_path(&self) -> String {
        format!(
            "{}/tags/{}/{}.json",
            self.namespace,
            T::REGISTRY_PATH,
            self.path
        )
    }
}

impl<T: TagRegistryType> std::fmt::Debug for TagKey<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TagKey({}:{})", self.namespace, self.path)
    }
}
