use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ResourceLocationBuf;

#[derive(Clone, Copy)]
pub struct ResourceLocation<'a> {
    namespace: &'a str,
    path: &'a str,
}

const DEFAULT_NAMESPACE: &str = "minecraft";

fn split_namespace(text: &str) -> (&str, &str) {
    match text.split_once(':') {
        Some(("", path)) => (DEFAULT_NAMESPACE, path),
        Some(parts) => parts,
        None => (DEFAULT_NAMESPACE, text),
    }
}

pub fn parse_parts(text: &str) -> Option<(&str, &str)> {
    let (namespace, path) = split_namespace(text);
    (valid_namespace(namespace.as_bytes()) && valid(path.as_bytes(), true))
        .then_some((namespace, path))
}

const fn valid_namespace(bytes: &[u8]) -> bool {
    !matches!(bytes, [b'.', b'.']) && valid(bytes, false)
}

const fn valid(mut bytes: &[u8], slash: bool) -> bool {
    while let [byte, rest @ ..] = bytes {
        if !(matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-')
            || slash && *byte == b'/')
        {
            return false;
        }
        bytes = rest;
    }
    true
}

pub(crate) fn check(namespace: &str, path: &str) -> Result<(), String> {
    if !valid_namespace(namespace.as_bytes()) {
        return Err(format!(
            "Non [a-z0-9_.-] character in namespace of identifier: {namespace}:{path}"
        ));
    }
    if namespace.len() > u16::MAX as usize {
        return Err(format!(
            "Namespace longer than {} bytes in identifier",
            u16::MAX
        ));
    }
    if !valid(path.as_bytes(), true) {
        return Err(format!(
            "Non [a-z0-9/._-] character in path of location: {namespace}:{path}"
        ));
    }
    Ok(())
}

impl ResourceLocation<'static> {
    pub const fn new_static(s: &'static str) -> Result<Self, &'static str> {
        let bytes = s.as_bytes();
        let mut colon = 0;
        let mut rest = bytes;
        while let [byte, tail @ ..] = rest {
            if *byte == b':' {
                break;
            }
            colon += 1;
            rest = tail;
        }
        let Some((namespace_bytes, [b':', path_bytes @ ..])) = bytes.split_at_checked(colon) else {
            return Err("ResourceLocation must contain ':'");
        };
        let (Ok(namespace), Ok(path)) = (
            core::str::from_utf8(namespace_bytes),
            core::str::from_utf8(path_bytes),
        ) else {
            return Err("resource location is not UTF-8");
        };
        if namespace.is_empty() {
            return Err("resource location namespace must not be empty");
        }
        if namespace.len() > u16::MAX as usize {
            return Err("resource location namespace is too long");
        }
        if !valid_namespace(namespace.as_bytes()) {
            return Err(
                "invalid character in resource location namespace (allowed: a-z 0-9 _ . -, not \"..\")",
            );
        }
        if !valid(path.as_bytes(), true) {
            return Err("invalid character in resource location path (allowed: a-z 0-9 _ . - /)");
        }
        Ok(ResourceLocation { namespace, path })
    }
}

impl<'a> ResourceLocation<'a> {
    pub fn new(namespace: &'a str, path: &'a str) -> Result<Self, InvalidResourceLocation> {
        check(namespace, path).map_err(|reason| InvalidResourceLocation {
            input: format!("{namespace}:{path}"),
            reason,
        })?;
        Ok(ResourceLocation { namespace, path })
    }

    pub fn read(text: &'a str) -> Result<Self, InvalidResourceLocation> {
        let (namespace, path) = split_namespace(text);
        check(namespace, path).map_err(|reason| InvalidResourceLocation {
            input: text.to_owned(),
            reason,
        })?;
        Ok(ResourceLocation { namespace, path })
    }

    pub(crate) const fn from_checked_parts(namespace: &'a str, path: &'a str) -> Self {
        ResourceLocation { namespace, path }
    }

    #[inline]
    pub const fn namespace(self) -> &'a str {
        self.namespace
    }

    #[inline]
    pub const fn path(self) -> &'a str {
        self.path
    }

    pub fn to_buf(self) -> ResourceLocationBuf {
        ResourceLocationBuf::from(self)
    }

    pub const fn len(self) -> usize {
        self.namespace.len() + 1 + self.path.len()
    }

    pub const fn is_empty(self) -> bool {
        false
    }
}

impl<'a> From<&ResourceLocation<'a>> for ResourceLocation<'a> {
    fn from(location: &ResourceLocation<'a>) -> Self {
        *location
    }
}

impl PartialEq for ResourceLocation<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.namespace == other.namespace && self.path == other.path
    }
}

impl Eq for ResourceLocation<'_> {}

impl PartialEq<str> for ResourceLocation<'_> {
    fn eq(&self, other: &str) -> bool {
        other
            .strip_prefix(self.namespace)
            .and_then(|rest| rest.strip_prefix(':'))
            == Some(self.path)
    }
}

impl PartialEq<&str> for ResourceLocation<'_> {
    fn eq(&self, other: &&str) -> bool {
        *self == **other
    }
}

impl Hash for ResourceLocation<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.namespace.hash(state);
        self.path.hash(state);
    }
}

impl PartialOrd for ResourceLocation<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResourceLocation<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.path, self.namespace).cmp(&(other.path, other.namespace))
    }
}

impl fmt::Display for ResourceLocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

impl fmt::Debug for ResourceLocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ResourceLocation(\"{}:{}\")", self.namespace, self.path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Not a valid resource location: {input} {reason}")]
pub struct InvalidResourceLocation {
    pub input: String,
    pub reason: String,
}

impl Serialize for ResourceLocation<'_> {
    fn serialize<Ser: Serializer>(&self, s: Ser) -> Result<Ser::Ok, Ser::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ResourceLocation<'de> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Borrowed;

        impl<'de> Visitor<'de> for Borrowed {
            type Value = ResourceLocation<'de>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a borrowed resource location")
            }

            fn visit_borrowed_str<E: de::Error>(self, text: &'de str) -> Result<Self::Value, E> {
                ResourceLocation::read(text).map_err(E::custom)
            }
        }

        d.deserialize_str(Borrowed)
    }
}

#[macro_export]
macro_rules! rl {
    ($s:literal) => {
        const {
            match $crate::ResourceLocation::new_static($s) {
                ::core::result::Result::Ok(location) => location,
                ::core::result::Result::Err(reason) => ::core::panic!("{}", reason),
            }
        }
    };
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use alloc::string::ToString;

    #[test]
    fn a_bare_path_reads_in_the_minecraft_namespace() {
        for (text, namespace, path) in [
            ("alt", "minecraft", "alt"),
            ("block/stone", "minecraft", "block/stone"),
            ("mcrs:alt", "mcrs", "alt"),
            (":alt", "minecraft", "alt"),
        ] {
            let location = ResourceLocation::read(text).unwrap();
            assert_eq!((location.namespace(), location.path()), (namespace, path));
        }
    }

    #[test]
    fn vanilla_character_sets_are_enforced() {
        assert_eq!(
            ResourceLocation::read("MC:alt").unwrap_err().to_string(),
            "Not a valid resource location: MC:alt Non [a-z0-9_.-] character in namespace of identifier: MC:alt"
        );
        assert_eq!(
            ResourceLocation::read("minecraft:Alt")
                .unwrap_err()
                .to_string(),
            "Not a valid resource location: minecraft:Alt Non [a-z0-9/._-] character in path of location: minecraft:Alt"
        );
        assert!(ResourceLocation::read("..:x").is_err());
        assert!(ResourceLocation::read("a:b:c").is_err());
    }

    #[test]
    fn new_static_enforces_the_character_sets() {
        let loc = crate::rl!("a.b-c_1:d/e.f-g_2");
        assert_eq!((loc.namespace(), loc.path()), ("a.b-c_1", "d/e.f-g_2"));
        for bad in [
            "alt",
            ":alt",
            "MC:alt",
            "minecraft:Alt",
            "a:b:c",
            "a/b:c",
            "..:alt",
        ] {
            assert!(ResourceLocation::new_static(bad).is_err(), "{bad}");
        }
        assert_eq!(crate::rl!("minecraft:").path(), "");
    }

    #[test]
    fn a_namespace_longer_than_the_offset_type_is_refused() {
        let namespace = "a".repeat(u16::MAX as usize + 1);
        assert!(ResourceLocation::read(&format!("{namespace}:x")).is_err());
        assert!(ResourceLocation::new(&namespace, "x").is_err());
        let longest = "a".repeat(u16::MAX as usize);
        let text = format!("{longest}:x");
        let kept = ResourceLocation::read(&text).unwrap();
        assert_eq!((kept.namespace().len(), kept.path()), (longest.len(), "x"));
    }

    #[test]
    fn a_borrowed_and_an_owned_location_are_one_value() {
        use core::hash::BuildHasher;
        let hasher = std::hash::RandomState::new();
        let bare = ResourceLocation::read("stone").unwrap();
        let owned = ResourceLocationBuf::read("minecraft:stone").unwrap();
        assert_eq!(bare, crate::rl!("minecraft:stone"));
        assert_eq!(owned, bare);
        assert_eq!(hasher.hash_one(bare), hasher.hash_one(&owned));
        assert_eq!(
            bare.cmp(&crate::rl!("minecraft:stone")),
            owned.as_location().cmp(&crate::rl!("minecraft:stone"))
        );
        assert!(bare == "minecraft:stone" && bare != "stone");
    }

    #[test]
    fn locations_order_by_path_then_namespace() {
        for (first, second) in [
            (crate::rl!("z:a"), crate::rl!("a:b")),
            (crate::rl!("a:x"), crate::rl!("b:x")),
            (crate::rl!("m:a"), crate::rl!("m:a_b")),
            (crate::rl!("m:a/b"), crate::rl!("m:a_b")),
        ] {
            assert!(first < second, "{first} sorts before {second}");
            assert!(first.to_buf() < second.to_buf(), "{first} as owned");
        }
    }

    #[test]
    fn the_sizes_stay_pinned() {
        assert_eq!(size_of::<ResourceLocation<'_>>(), 32, "two &str");
        assert_eq!(
            size_of::<ResourceLocationBuf>(),
            24,
            "a Box<str> and the colon"
        );
    }
}
