use serde::{Deserialize, Serialize};

use crate::{Artifact, verify};

const OBJECTS: &str = "https://resources.download.minecraft.net";

/// A file of the asset index layer, which sits above the jar. Pinned to the release like
/// `release.json` is: the index lists each object by SHA-1 and size.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetObject<'a> {
    /// Where the file lies below `assets/` once it is laid over the jar's files.
    pub path: &'a str,
    pub sha1: &'a str,
    pub size: u64,
}

/// The objects of the font folder of the asset index, which the update tool writes to
/// `font_objects.json`: the real `include/unifont.json`, which replaces the jar's empty
/// placeholder, and the Unifont archives it names.
pub const FONT_OBJECTS: &[AssetObject<'static>] =
    &include!(concat!(env!("OUT_DIR"), "/font_objects.rs"));

impl AssetObject<'_> {
    pub fn url(&self) -> String {
        format!("{OBJECTS}/{}/{}", self.directory(), self.sha1)
    }

    /// The first two digits of the SHA-1, the folder the launcher keeps the object in.
    pub fn directory(&self) -> &str {
        self.sha1.get(..2).unwrap_or_default()
    }

    pub fn holds(&self, bytes: &[u8]) -> bool {
        verify(
            &Artifact {
                url: &self.url(),
                sha1: self.sha1,
                size: self.size,
            },
            bytes,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_has_the_address_of_its_digest() {
        let object = AssetObject {
            path: "minecraft/font/a.zip",
            sha1: "ccd5ac4767ce0a9c71d1dd62f2dc25449789b5dd",
            size: 3,
        };
        assert_eq!(
            object.url(),
            "https://resources.download.minecraft.net/cc/ccd5ac4767ce0a9c71d1dd62f2dc25449789b5dd"
        );
        assert!(!object.holds(b"not the archive"));
    }

    #[test]
    fn the_pinned_objects_are_font_files() {
        assert!(!FONT_OBJECTS.is_empty());
        for object in FONT_OBJECTS {
            assert!(object.path.starts_with("minecraft/font/"), "{object:?}");
            assert_eq!(object.sha1.len(), 40, "{object:?}");
        }
    }
}
