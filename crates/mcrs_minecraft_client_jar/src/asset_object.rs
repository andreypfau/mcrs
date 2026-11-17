use serde::{Deserialize, Serialize};

use crate::sha1_hex;

pub const OBJECTS: &str = "https://resources.download.minecraft.net";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetObject<'a> {
    pub path: &'a str,
    pub sha1: &'a str,
    pub size: u64,
}

pub const FONT_OBJECTS: &[AssetObject<'static>] =
    &include!(concat!(env!("OUT_DIR"), "/font_objects.rs"));

impl AssetObject<'_> {
    pub fn url(&self, objects: &str) -> String {
        let objects = objects.trim_end_matches('/');
        format!("{objects}/{}/{}", self.directory(), self.sha1)
    }

    pub fn directory(&self) -> &str {
        self.sha1.get(..2).unwrap_or_default()
    }

    pub fn holds(&self, bytes: &[u8]) -> bool {
        bytes.len() as u64 == self.size && sha1_hex(bytes) == self.sha1
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
            object.url(OBJECTS),
            "https://resources.download.minecraft.net/cc/ccd5ac4767ce0a9c71d1dd62f2dc25449789b5dd"
        );
        assert_eq!(
            object.url("https://studio.example/objects/"),
            "https://studio.example/objects/cc/ccd5ac4767ce0a9c71d1dd62f2dc25449789b5dd"
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
