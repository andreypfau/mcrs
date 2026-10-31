use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{InvalidResourceLocation, ResourceLocation};

/// An owned resource location, always held as its canonical `namespace:path` text.
///
/// [`as_location`](Self::as_location) lends the borrowed form. A clone copies the text, so code
/// that runs per tick holds a numeric registry id instead.
#[derive(Clone)]
pub struct ResourceLocationBuf {
    string: Box<str>,
    colon: u16,
}

impl ResourceLocationBuf {
    /// Both halves are checked and nothing is defaulted, so an empty namespace stays empty.
    pub fn new(namespace: &str, path: &str) -> Result<Self, InvalidResourceLocation> {
        ResourceLocation::new(namespace, path).map(Self::from)
    }

    pub fn minecraft(path: &str) -> Result<Self, InvalidResourceLocation> {
        Self::new("minecraft", path)
    }

    /// A missing or empty namespace is `minecraft`, as [`ResourceLocation::read`] reads it.
    pub fn read(text: &str) -> Result<Self, InvalidResourceLocation> {
        ResourceLocation::read(text).map(Self::from)
    }

    /// The canonical `namespace:path` text.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.string
    }

    #[inline]
    pub fn namespace(&self) -> &str {
        self.as_location().namespace()
    }

    #[inline]
    pub fn path(&self) -> &str {
        self.as_location().path()
    }

    /// The borrowed form. The name is not `as_ref`, which returns a reference.
    #[inline]
    pub fn as_location(&self) -> ResourceLocation<'_> {
        let (namespace, path) = self
            .string
            .split_at_checked(usize::from(self.colon))
            .and_then(|(namespace, rest)| Some((namespace, rest.strip_prefix(':')?)))
            .unwrap_or(("", &self.string));
        ResourceLocation::from_checked_parts(namespace, path)
    }
}

impl From<ResourceLocation<'_>> for ResourceLocationBuf {
    fn from(location: ResourceLocation<'_>) -> Self {
        let namespace = location.namespace();
        Self {
            string: format!("{namespace}:{}", location.path()).into_boxed_str(),
            // A location is checked on construction, so its namespace fits the offset.
            colon: u16::try_from(namespace.len()).unwrap_or(u16::MAX),
        }
    }
}

impl<'a> From<&'a ResourceLocationBuf> for ResourceLocation<'a> {
    fn from(location: &'a ResourceLocationBuf) -> Self {
        location.as_location()
    }
}

/// Looks an owned key up by a borrowed location, in a map such as `hashbrown`'s.
impl equivalent::Equivalent<ResourceLocationBuf> for ResourceLocation<'_> {
    fn equivalent(&self, key: &ResourceLocationBuf) -> bool {
        *self == key.as_location()
    }
}

impl From<&ResourceLocationBuf> for ResourceLocationBuf {
    fn from(location: &ResourceLocationBuf) -> Self {
        location.clone()
    }
}

impl core::str::FromStr for ResourceLocationBuf {
    type Err = InvalidResourceLocation;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::read(text)
    }
}

impl PartialEq for ResourceLocationBuf {
    fn eq(&self, other: &Self) -> bool {
        self.string == other.string
    }
}

impl Eq for ResourceLocationBuf {}

impl PartialEq<ResourceLocation<'_>> for ResourceLocationBuf {
    fn eq(&self, other: &ResourceLocation<'_>) -> bool {
        self.as_location() == *other
    }
}

impl PartialEq<ResourceLocationBuf> for ResourceLocation<'_> {
    fn eq(&self, other: &ResourceLocationBuf) -> bool {
        *self == other.as_location()
    }
}

impl PartialEq<ResourceLocation<'_>> for &ResourceLocationBuf {
    fn eq(&self, other: &ResourceLocation<'_>) -> bool {
        self.as_location() == *other
    }
}

impl PartialEq<str> for ResourceLocationBuf {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for ResourceLocationBuf {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl Hash for ResourceLocationBuf {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_location().hash(state);
    }
}

impl PartialOrd for ResourceLocationBuf {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResourceLocationBuf {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_location().cmp(&other.as_location())
    }
}

impl fmt::Display for ResourceLocationBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.string)
    }
}

impl fmt::Debug for ResourceLocationBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceLocation({:?})", &*self.string)
    }
}

impl Serialize for ResourceLocationBuf {
    fn serialize<Ser: Serializer>(&self, s: Ser) -> Result<Ser::Ok, Ser::Error> {
        s.serialize_str(&self.string)
    }
}

impl<'de> Deserialize<'de> for ResourceLocationBuf {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::read(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trips_the_canonical_text_in_both_forms() {
        for text in ["stone", "minecraft:stone", "mcrs:block/alt"] {
            let owned = ResourceLocationBuf::read(text).unwrap();
            let json = serde_json::to_string(&owned).unwrap();
            assert_eq!(
                serde_json::from_str::<ResourceLocationBuf>(&json).unwrap(),
                owned
            );
            let borrowed: ResourceLocation<'_> = serde_json::from_str(&json).unwrap();
            assert_eq!(borrowed, owned);
            assert_eq!(serde_json::to_string(&borrowed).unwrap(), json);
        }
    }
}
