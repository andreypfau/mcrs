use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::{Borrow, Cow};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A namespaced resource identifier in the form `namespace:path`.
///
/// Generic over the string storage type `S`:
/// - `ResourceLocation<Arc<str>>` (the default) — heap-allocated, cheap to clone.
/// - `ResourceLocation<&'static str>` — zero-alloc, `Copy`, const-constructible.
/// - `ResourceLocation<Cow<'a, str>>` — borrowed or owned, used by the wire codecs.
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
}

// ─── &'static str constructors ───────────────────────────────────────────────

impl ResourceLocation<&'static str> {
    /// Const constructor; panics unless `s` is `namespace:path` with a
    /// non-empty namespace in `[a-z0-9_.-]` and a non-empty path in
    /// `[a-z0-9_.-/]`. Inside `rl!` the panic is a compile error.
    #[track_caller]
    pub const fn new_static(s: &'static str) -> Self {
        let bytes = s.as_bytes();
        let mut colon = None;
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            match colon {
                None if c == b':' => colon = Some(i),
                None => {
                    if !matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-') {
                        panic!(
                            "invalid character in resource location namespace (allowed: a-z 0-9 _ . -)"
                        );
                    }
                }
                Some(_) => {
                    if !matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/') {
                        panic!(
                            "invalid character in resource location path (allowed: a-z 0-9 _ . - /)"
                        );
                    }
                }
            }
            i += 1;
        }
        match colon {
            Some(0) => panic!("resource location namespace must not be empty"),
            Some(pos) if pos + 1 == bytes.len() => {
                panic!("resource location path must not be empty")
            }
            Some(pos) => ResourceLocation {
                string: s,
                colon_pos: pos as u16,
            },
            None => panic!("ResourceLocation must contain ':'"),
        }
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

    /// `Identifier.read`: a missing or empty namespace is `minecraft`, and
    /// both halves are checked against vanilla's character sets.
    pub fn read(s: &str) -> Result<Self, InvalidResourceLocation> {
        let (namespace, path) = match s.split_once(':') {
            Some(("", path)) => ("minecraft", path),
            Some((namespace, path)) => (namespace, path),
            None => ("minecraft", s),
        };
        let invalid = |reason: String| InvalidResourceLocation {
            input: s.to_owned(),
            reason,
        };
        if namespace == ".."
            || !namespace
                .chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
        {
            return Err(invalid(format!(
                "Non [a-z0-9_.-] character in namespace of identifier: {namespace}:{path}"
            )));
        }
        if !path
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/'))
        {
            return Err(invalid(format!(
                "Non [a-z0-9/._-] character in path of location: {namespace}:{path}"
            )));
        }
        Ok(ResourceLocation::new(namespace, path))
    }
}

// ─── Cow<str> constructors ───────────────────────────────────────────────────

impl<'a> ResourceLocation<Cow<'a, str>> {
    /// Parse a `namespace:path` string. Returns an error if `:` is missing.
    pub fn parse_cow(s: impl Into<Cow<'a, str>>) -> Result<Self, ResourceLocationError> {
        let string = s.into();
        match string.find(':') {
            Some(pos) => Ok(ResourceLocation {
                string,
                colon_pos: pos as u16,
            }),
            None => Err(ResourceLocationError(string.into_owned())),
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

impl From<ResourceLocation<&'static str>> for ResourceLocation<Cow<'static, str>> {
    #[inline]
    fn from(rl: ResourceLocation<&'static str>) -> Self {
        ResourceLocation {
            string: Cow::Borrowed(rl.string),
            colon_pos: rl.colon_pos,
        }
    }
}

impl<'a> From<ResourceLocation<Cow<'a, str>>> for ResourceLocation<Arc<str>> {
    #[inline]
    fn from(rl: ResourceLocation<Cow<'a, str>>) -> Self {
        ResourceLocation {
            string: Arc::from(rl.string.as_ref()),
            colon_pos: rl.colon_pos,
        }
    }
}

impl From<ResourceLocation<Arc<str>>> for ResourceLocation<Cow<'static, str>> {
    #[inline]
    fn from(rl: ResourceLocation<Arc<str>>) -> Self {
        ResourceLocation {
            string: Cow::Owned(rl.string.to_string()),
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

#[derive(Debug, thiserror::Error)]
#[error("Not a valid resource location: {input} {reason}")]
pub struct InvalidResourceLocation {
    pub input: String,
    pub reason: String,
}

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
        ResourceLocation::read(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ResourceLocation<Cow<'static, str>> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        ResourceLocation::<Arc<str>>::deserialize(d).map(Into::into)
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

/// A `ResourceLocation<&'static str>` from a `namespace:path` literal,
/// validated at compile time.
#[macro_export]
macro_rules! rl {
    ($s:literal) => {
        const { $crate::resource_location::ResourceLocation::new_static($s) }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_path_reads_in_the_minecraft_namespace() {
        let read = |json: &str| serde_json::from_str::<ResourceLocation<Arc<str>>>(json).unwrap();
        assert_eq!(read(r#""alt""#).as_str(), "minecraft:alt");
        assert_eq!(read(r#""block/stone""#).as_str(), "minecraft:block/stone");
        assert_eq!(read(r#""mcrs:alt""#).as_str(), "mcrs:alt");
        assert_eq!(read(r#"":alt""#).as_str(), "minecraft:alt");
        assert!(ResourceLocation::parse("alt").is_err());
    }

    #[test]
    fn vanilla_character_sets_are_enforced() {
        let read = |json: &str| serde_json::from_str::<ResourceLocation<Arc<str>>>(json);
        assert_eq!(
            read(r#""MC:alt""#).unwrap_err().to_string(),
            "Not a valid resource location: MC:alt Non [a-z0-9_.-] character in namespace of identifier: MC:alt"
        );
        assert_eq!(
            read(r#""minecraft:Alt""#).unwrap_err().to_string(),
            "Not a valid resource location: minecraft:Alt Non [a-z0-9/._-] character in path of location: minecraft:Alt"
        );
        assert!(read(r#""..:x""#).is_err());
        assert!(read(r#""a:b:c""#).is_err());
        assert_eq!(
            read(r#""a.b-c_1:d/e.f-g_2""#).unwrap().as_str(),
            "a.b-c_1:d/e.f-g_2"
        );
    }

    #[test]
    fn new_static_enforces_the_character_sets() {
        let loc = crate::rl!("a.b-c_1:d/e.f-g_2");
        assert_eq!((loc.namespace(), loc.path()), ("a.b-c_1", "d/e.f-g_2"));
        for bad in [
            "alt",
            ":alt",
            "minecraft:",
            "MC:alt",
            "minecraft:Alt",
            "a:b:c",
            "a/b:c",
        ] {
            assert!(
                std::panic::catch_unwind(|| ResourceLocation::new_static(bad)).is_err(),
                "{bad}"
            );
        }
    }
}
