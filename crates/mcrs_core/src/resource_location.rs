use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

/// A namespaced resource identifier in the form `namespace:path`.
///
/// Cheap to clone — backed by a single `Arc<str>`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ResourceLocation(Arc<str>);

impl ResourceLocation {
    /// Create a new `ResourceLocation` from a namespace and path.
    pub fn new(namespace: &str, path: &str) -> Self {
        ResourceLocation(format!("{namespace}:{path}").into())
    }

    /// Shortcut for `ResourceLocation::new("minecraft", path)`.
    pub fn minecraft(path: &str) -> Self {
        ResourceLocation::new("minecraft", path)
    }

    /// The namespace portion (everything before `:`).
    pub fn namespace(&self) -> &str {
        self.0.split_once(':').map(|(ns, _)| ns).unwrap_or("")
    }

    /// The path portion (everything after `:`).
    pub fn path(&self) -> &str {
        self.0.split_once(':').map(|(_, p)| p).unwrap_or(&self.0)
    }

    /// The full `namespace:path` string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceLocation({:?})", self.0.as_ref())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("missing ':' separator in ResourceLocation: {0:?}")]
pub struct ResourceLocationError(String);

impl FromStr for ResourceLocation {
    type Err = ResourceLocationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(':') {
            Ok(ResourceLocation(Arc::from(s)))
        } else {
            Err(ResourceLocationError(s.to_owned()))
        }
    }
}

impl Serialize for ResourceLocation {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ResourceLocation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ResourceLocation::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Macro for creating a `ResourceLocation` from a `"namespace:path"` literal.
///
/// ```rust
/// use mcrs_core::rl;
/// let loc = rl!("minecraft:stone");
/// ```
#[macro_export]
macro_rules! rl {
    ($s:literal) => {
        $crate::resource_location::ResourceLocation::from_str_const($s)
    };
}

impl ResourceLocation {
    /// Const-compatible constructor — panics at compile time if `:` is absent.
    /// Prefer the `rl!` macro for ergonomics.
    #[track_caller]
    pub fn from_str_const(s: &'static str) -> Self {
        assert!(s.contains(':'), "ResourceLocation must contain ':'");
        ResourceLocation(Arc::from(s))
    }
}
