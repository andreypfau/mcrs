use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A namespaced resource identifier in the form `namespace:path`.
///
/// Generic over the string storage type `S`:
/// - `ResourceLocation<Arc<str>>` (the default) — heap-allocated, cheap to clone.
/// - `ResourceLocation<&'static str>` — zero-alloc, `Copy`, const-constructible.
///
/// Both variants hash and compare identically, and `ResourceLocation<&'static str>`
/// can be used for zero-allocation lookups into `HashMap<ResourceLocation, …>` via
/// the `Borrow<str>` impl.
#[derive(Clone)]
pub struct ResourceLocation<S = Arc<str>> {
    string: S,
    colon_pos: u16,
}

// Copy for &'static str variant
impl Copy for ResourceLocation<&'static str> {}

// ─── Common methods for any S: AsRef<str> ────────────────────────────────────

impl<S: AsRef<str>> ResourceLocation<S> {
    /// The full `namespace:path` string.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.string.as_ref()
    }

    /// The namespace portion (everything before `:`).
    #[inline]
    pub fn namespace(&self) -> &str {
        &self.string.as_ref()[..self.colon_pos as usize]
    }

    /// The path portion (everything after `:`).
    #[inline]
    pub fn path(&self) -> &str {
        &self.string.as_ref()[(self.colon_pos as usize + 1)..]
    }

    /// Convert to the heap-allocated variant.
    pub fn to_arc(&self) -> ResourceLocation<Arc<str>> {
        ResourceLocation {
            string: Arc::from(self.string.as_ref()),
            colon_pos: self.colon_pos,
        }
    }

    /// Build the asset path for this resource location.
    ///
    /// Format: `{namespace}/{path}` (e.g. `"minecraft/stone"`).
    pub fn to_asset_path(&self) -> String {
        format!("{}/{}", self.namespace(), self.path())
    }
}

// ─── &'static str constructors ───────────────────────────────────────────────

impl ResourceLocation<&'static str> {
    /// The full `namespace:path` string with `'static` lifetime.
    #[inline]
    pub const fn as_static_str(&self) -> &'static str {
        self.string
    }

    /// The namespace portion with `'static` lifetime.
    #[inline]
    pub fn namespace_static(&self) -> &'static str {
        let s = self.string;
        // SAFETY: colon_pos is always a valid byte index into s
        &s[..self.colon_pos as usize]
    }

    /// The path portion with `'static` lifetime.
    #[inline]
    pub fn path_static(&self) -> &'static str {
        let s = self.string;
        &s[(self.colon_pos as usize + 1)..]
    }

    /// Const-compatible constructor from a pre-validated string.
    ///
    /// # Safety contract (not unsafe, but panics on bad input)
    /// The caller must ensure `s` contains exactly one `:` separator.
    /// Prefer the `rl!` macro which validates at compile time.
    #[track_caller]
    pub const fn new_static(s: &'static str) -> Self {
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut colon = None;
        while i < bytes.len() {
            if bytes[i] == b':' {
                colon = Some(i);
                break;
            }
            i += 1;
        }
        match colon {
            Some(pos) => ResourceLocation {
                string: s,
                colon_pos: pos as u16,
            },
            None => panic!("ResourceLocation must contain ':'"),
        }
    }

    /// Used by the `rl!` macro — do not call directly.
    #[doc(hidden)]
    #[inline]
    pub const fn __from_validated(string: &'static str, colon_pos: u16) -> Self {
        ResourceLocation { string, colon_pos }
    }
}

// ─── Arc<str> constructors ───────────────────────────────────────────────────

impl ResourceLocation<Arc<str>> {
    /// Create a new `ResourceLocation` from a namespace and path.
    pub fn new(namespace: &str, path: &str) -> Self {
        let full = format!("{namespace}:{path}");
        let colon_pos = namespace.len() as u16;
        ResourceLocation {
            string: Arc::from(full.as_str()),
            colon_pos,
        }
    }

    /// Shortcut for `ResourceLocation::new("minecraft", path)`.
    pub fn minecraft(path: &str) -> Self {
        ResourceLocation::new("minecraft", path)
    }

    /// Parse a `namespace:path` string. Returns an error if `:` is missing.
    pub fn parse(s: &str) -> Result<Self, ResourceLocationError> {
        match s.find(':') {
            Some(pos) => Ok(ResourceLocation {
                string: Arc::from(s),
                colon_pos: pos as u16,
            }),
            None => Err(ResourceLocationError(s.to_owned())),
        }
    }
}

// ─── Conversions ─────────────────────────────────────────────────────────────

impl From<ResourceLocation<&'static str>> for ResourceLocation<Arc<str>> {
    #[inline]
    fn from(rl: ResourceLocation<&'static str>) -> Self {
        ResourceLocation {
            string: Arc::from(rl.string),
            colon_pos: rl.colon_pos,
        }
    }
}

// ─── Trait impls ─────────────────────────────────────────────────────────────

impl<S: AsRef<str>> Hash for ResourceLocation<S> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.string.as_ref().hash(state);
    }
}

impl<S: AsRef<str>, T: AsRef<str>> PartialEq<ResourceLocation<T>> for ResourceLocation<S> {
    #[inline]
    fn eq(&self, other: &ResourceLocation<T>) -> bool {
        self.string.as_ref() == other.string.as_ref()
    }
}

impl<S: AsRef<str>> Eq for ResourceLocation<S> {}

impl<S: AsRef<str>> PartialOrd for ResourceLocation<S> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: AsRef<str>> Ord for ResourceLocation<S> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.string.as_ref().cmp(other.string.as_ref())
    }
}

/// Enables zero-alloc lookup in `HashMap<ResourceLocation, …>` via `map.get(rl.as_str())`.
impl<S: AsRef<str>> Borrow<str> for ResourceLocation<S> {
    #[inline]
    fn borrow(&self) -> &str {
        self.string.as_ref()
    }
}

impl<S: AsRef<str>> fmt::Display for ResourceLocation<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_ref())
    }
}

impl<S: AsRef<str>> fmt::Debug for ResourceLocation<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceLocation({:?})", self.string.as_ref())
    }
}

// ─── FromStr ─────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
#[error("missing ':' separator in ResourceLocation: {0:?}")]
pub struct ResourceLocationError(pub String);

impl std::str::FromStr for ResourceLocation<Arc<str>> {
    type Err = ResourceLocationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ResourceLocation::parse(s)
    }
}

// ─── Serde ───────────────────────────────────────────────────────────────────

impl<S: AsRef<str>> Serialize for ResourceLocation<S> {
    fn serialize<Ser: Serializer>(&self, s: Ser) -> Result<Ser::Ok, Ser::Error> {
        s.serialize_str(self.string.as_ref())
    }
}

impl<'de> Deserialize<'de> for ResourceLocation<Arc<str>> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ResourceLocation::parse(&s).map_err(serde::de::Error::custom)
    }
}

// ─── Backwards compat ────────────────────────────────────────────────────────

impl ResourceLocation<Arc<str>> {
    /// Const-compatible constructor — panics at compile time if `:` is absent.
    /// Prefer the `rl!` macro for ergonomics.
    #[track_caller]
    pub fn from_str_const(s: &'static str) -> Self {
        ResourceLocation::new_static(s).to_arc()
    }
}

// ─── rl! macro ───────────────────────────────────────────────────────────────

/// Macro for creating a `ResourceLocation<&'static str>` from a string literal.
///
/// Validates the resource location at compile time (charset, format) and
/// auto-prefixes `"minecraft:"` when no namespace is given.
///
/// ```rust,ignore
/// use mcrs_core::rl;
/// let loc = rl!("minecraft:stone");   // ResourceLocation<&'static str>
/// let loc2 = rl!("stone");            // same: "minecraft:stone"
/// ```
#[macro_export]
macro_rules! rl {
    ($s:literal) => {{
        const _VALIDATED: (&str, u16) = $crate::__rl_impl!($s);
        $crate::resource_location::ResourceLocation::__from_validated(_VALIDATED.0, _VALIDATED.1)
    }};
}

// Re-export the proc macro under a hidden name for use by the rl! declarative macro.
#[doc(hidden)]
pub use mcrs_core_macros::rl_impl as __rl_impl;
