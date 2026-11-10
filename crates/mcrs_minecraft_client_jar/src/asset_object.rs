use crate::sha1_hex;

const OBJECTS: &str = "https://resources.download.minecraft.net";

/// A file of the asset index layer, which sits above the jar. Pinned to the release like
/// `release.json` is: the index lists each object by SHA-1 and size.
#[derive(Clone, Copy, Debug)]
pub struct AssetObject {
    /// Where the file lies below `assets/` once it is laid over the jar's files.
    pub path: &'static str,
    pub sha1: &'static str,
    pub size: u64,
}

/// The objects the default font needs beyond the jar: the real `include/unifont.json`, which
/// replaces the jar's empty placeholder, and the Unifont archives it names.
pub const FONT_OBJECTS: [AssetObject; 3] = [
    AssetObject {
        path: "minecraft/font/include/unifont.json",
        sha1: "402ded0eebd448033ef415e861a17513075f80e7",
        size: 3993,
    },
    AssetObject {
        path: "minecraft/font/unifont.zip",
        sha1: "ccd5ac4767ce0a9c71d1dd62f2dc25449789b5dd",
        size: 1_559_654,
    },
    AssetObject {
        path: "minecraft/font/unifont_jp.zip",
        sha1: "590470ab0f17afb73a4e41d9cb56fdbe069d275a",
        size: 234_900,
    },
];

impl AssetObject {
    pub fn url(&self) -> String {
        format!("{OBJECTS}/{}/{}", self.directory(), self.sha1)
    }

    /// The first two digits of the SHA-1, the folder the launcher keeps the object in.
    pub fn directory(&self) -> &'static str {
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
        let object = FONT_OBJECTS[1];
        assert_eq!(
            object.url(),
            "https://resources.download.minecraft.net/cc/ccd5ac4767ce0a9c71d1dd62f2dc25449789b5dd"
        );
        assert!(!object.holds(b"not the archive"));
    }
}
