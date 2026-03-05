use crate::resource_location::ResourceLocation;
use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A typed resource key — a `ResourceLocation` annotated with a phantom type
/// indicating which registry it belongs to.
///
/// Generic over `S` (string storage), just like `ResourceLocation<S>`:
/// - `ResourceKey<T, &'static str>` — `Copy`, const-constructible.
/// - `ResourceKey<T, Arc<str>>` (default) — `Clone`, deserializable.
///
/// Both variants hash and compare identically (by underlying string), and the
/// `Borrow<str>` impl enables zero-alloc lookups in `HashMap<ResourceKey<T>, …>`.
pub struct ResourceKey<T, S = Arc<str>> {
    location: ResourceLocation<S>,
    _marker: PhantomData<fn() -> T>,
}

// ─── Copy for &'static str variant ──────────────────────────────────────────

impl<T> Copy for ResourceKey<T, &'static str> {}

// ─── Manual Clone (no bound on T) ───────────────────────────────────────────

impl<T, S: Clone> Clone for ResourceKey<T, S> {
    fn clone(&self) -> Self {
        ResourceKey {
            location: self.location.clone(),
            _marker: PhantomData,
        }
    }
}

// ─── Constructors ───────────────────────────────────────────────────────────

impl<T> ResourceKey<T, &'static str> {
    /// Const-compatible constructor from a static `ResourceLocation`.
    ///
    /// ```rust,ignore
    /// use mcrs_core::{rl, ResourceKey};
    /// const KEY: ResourceKey<MyType, &'static str> = ResourceKey::new(rl!("minecraft:overworld"));
    /// ```
    pub const fn new(location: ResourceLocation<&'static str>) -> Self {
        ResourceKey {
            location,
            _marker: PhantomData,
        }
    }
}

impl<T> ResourceKey<T, Arc<str>> {
    /// Runtime constructor from an `Arc<str>` resource location.
    pub fn from_location(location: ResourceLocation<Arc<str>>) -> Self {
        ResourceKey {
            location,
            _marker: PhantomData,
        }
    }
}

// ─── Common accessors ───────────────────────────────────────────────────────

impl<T, S: AsRef<str>> ResourceKey<T, S> {
    /// The full `namespace:path` string.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.location.as_str()
    }

    /// The underlying `ResourceLocation`.
    #[inline]
    pub fn location(&self) -> &ResourceLocation<S> {
        &self.location
    }

    /// The namespace portion.
    #[inline]
    pub fn namespace(&self) -> &str {
        self.location.namespace()
    }

    /// The path portion.
    #[inline]
    pub fn path(&self) -> &str {
        self.location.path()
    }
}

// ─── Conversion: static → Arc ───────────────────────────────────────────────

impl<T> From<ResourceKey<T, &'static str>> for ResourceKey<T, Arc<str>> {
    fn from(key: ResourceKey<T, &'static str>) -> Self {
        ResourceKey {
            location: key.location.into(),
            _marker: PhantomData,
        }
    }
}

// ─── Hash (delegates to underlying string) ──────────────────────────────────

impl<T, S: AsRef<str>> Hash for ResourceKey<T, S> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.location.as_str().hash(state);
    }
}

// ─── Cross-variant PartialEq ────────────────────────────────────────────────

impl<T, S: AsRef<str>, U: AsRef<str>> PartialEq<ResourceKey<T, U>> for ResourceKey<T, S> {
    #[inline]
    fn eq(&self, other: &ResourceKey<T, U>) -> bool {
        self.location.as_str() == other.location.as_str()
    }
}

impl<T, S: AsRef<str>> Eq for ResourceKey<T, S> {}

// ─── Ord ────────────────────────────────────────────────────────────────────

impl<T, S: AsRef<str>> PartialOrd for ResourceKey<T, S> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T, S: AsRef<str>> Ord for ResourceKey<T, S> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.location.as_str().cmp(other.location.as_str())
    }
}

// ─── Borrow<str> for zero-alloc HashMap lookups ─────────────────────────────

impl<T, S: AsRef<str>> Borrow<str> for ResourceKey<T, S> {
    #[inline]
    fn borrow(&self) -> &str {
        self.location.as_str()
    }
}

// ─── Display / Debug ────────────────────────────────────────────────────────

impl<T, S: AsRef<str>> fmt::Display for ResourceKey<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.location.as_str())
    }
}

impl<T, S: AsRef<str>> fmt::Debug for ResourceKey<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ResourceKey<{}>({})",
            std::any::type_name::<T>(),
            self.location.as_str()
        )
    }
}

// ─── Serde ──────────────────────────────────────────────────────────────────

impl<T, S: AsRef<str>> Serialize for ResourceKey<T, S> {
    fn serialize<Ser: Serializer>(&self, s: Ser) -> Result<Ser::Ok, Ser::Error> {
        s.serialize_str(self.location.as_str())
    }
}

impl<'de, T> Deserialize<'de> for ResourceKey<T, Arc<str>> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let location = ResourceLocation::<Arc<str>>::deserialize(d)?;
        Ok(ResourceKey {
            location,
            _marker: PhantomData,
        })
    }
}
