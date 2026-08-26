use std::sync::Arc;

use bevy_asset::{Handle, LoadContext};

use crate::resource_location::ResourceLocation;
use crate::tag::file::{TagFile, TagFileSettings};
use crate::tag::key::{TagKey, TaggedRegistry};

/// A loaded reference to a tag in a typed registry.
///
/// Analogous to Minecraft Java's `TagKey<T>` when used inside asset-loaded
/// structures (e.g. `DimensionType.infiniburn`).
///
/// Stores both the typed [`TagKey`] and the [`Handle<TagFile>`] for the tag's
/// asset data, so the tag file is automatically loaded as a sub-asset
/// dependency by Bevy's asset system.
///
/// # Construction
///
/// Use [`TagRef::load`] inside an `AssetLoader` to parse a tag reference
/// string and load the corresponding tag file in one step:
///
/// ```rust,ignore
/// let tag = TagRef::<Block>::load("minecraft:infiniburn_overworld", load_context)?;
/// ```
pub struct TagRef<T: TaggedRegistry> {
    key: TagKey<T, Arc<str>>,
    handle: Handle<TagFile>,
}

// Manual impls so T doesn't need Clone/Debug.
impl<T: TaggedRegistry> Clone for TagRef<T> {
    fn clone(&self) -> Self {
        TagRef {
            key: self.key.clone(),
            handle: self.handle.clone(),
        }
    }
}

impl<T: TaggedRegistry> std::fmt::Debug for TagRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TagRef")
            .field("key", &self.key)
            .field("handle", &self.handle)
            .finish()
    }
}

impl<T: TaggedRegistry> TagRef<T> {
    /// Parse a `namespace:path` string (without `#` prefix) and load the
    /// corresponding tag file as a sub-asset.
    ///
    /// The registry path segment (e.g. `"block"`) is derived from
    /// `T::REGISTRY_PATH`, so the correct `tags/{segment}/…` path is used
    /// automatically.
    pub fn load(
        rl_str: &str,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self, crate::resource_location::ResourceLocationError> {
        let rl: ResourceLocation<Arc<str>> = ResourceLocation::parse(rl_str)?;
        let key = TagKey::from_location(rl);
        let handle = load_context
            .load_builder()
            .with_settings(move |s: &mut TagFileSettings| {
                s.registry_segment = T::REGISTRY_PATH.to_string();
            })
            .load::<TagFile>(key.asset_path());
        Ok(TagRef { key, handle })
    }

    /// The typed tag key.
    #[inline]
    pub fn key(&self) -> &TagKey<T, Arc<str>> {
        &self.key
    }

    /// The loaded tag file handle.
    #[inline]
    pub fn handle(&self) -> &Handle<TagFile> {
        &self.handle
    }
}
