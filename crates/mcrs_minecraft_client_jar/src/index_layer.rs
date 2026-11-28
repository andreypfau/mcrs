use crate::{AssetObject, Files};

const FONTS: &str = "minecraft/font/";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexLayer {
    pub files: Files,
    pub failures: Vec<ObjectFailure>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectFailure {
    pub path: &'static str,
    pub reason: String,
}

impl IndexLayer {
    pub(crate) fn gather(
        results: impl IntoIterator<Item = (&'static AssetObject<'static>, Result<Vec<u8>, String>)>,
    ) -> Self {
        let mut layer = Self::default();
        for (object, result) in results {
            match result {
                Ok(bytes) => layer.files.push((object.path.to_owned(), bytes)),
                Err(reason) => layer.failures.push(ObjectFailure {
                    path: object.path,
                    reason,
                }),
            }
        }
        if layer
            .failures
            .iter()
            .any(|failure| failure.path.starts_with(FONTS))
        {
            layer.files.retain(|(path, _)| !path.starts_with(FONTS));
        }
        layer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_JSON: AssetObject<'static> = AssetObject {
        path: "minecraft/font/include/unifont.json",
        sha1: "",
        size: 0,
    };
    const FONT_ZIP: AssetObject<'static> = AssetObject {
        path: "minecraft/font/unifont.zip",
        sha1: "",
        size: 0,
    };
    const FACE: AssetObject<'static> = AssetObject {
        path: "minecraft/textures/gui/title/background/panorama_0.png",
        sha1: "",
        size: 0,
    };
    const OVERLAY: AssetObject<'static> = AssetObject {
        path: "minecraft/textures/gui/title/background/panorama_overlay.png",
        sha1: "",
        size: 0,
    };

    fn paths(layer: &IndexLayer) -> Vec<&str> {
        layer.files.iter().map(|(path, _)| path.as_str()).collect()
    }

    #[test]
    fn a_failed_face_costs_only_that_face() {
        let layer = IndexLayer::gather([
            (&FONT_JSON, Ok(vec![1])),
            (&FACE, Err("offline".to_owned())),
            (&FONT_ZIP, Ok(vec![2])),
            (&OVERLAY, Ok(vec![3])),
        ]);
        assert_eq!(paths(&layer), [FONT_JSON.path, FONT_ZIP.path, OVERLAY.path]);
        assert_eq!(
            layer.failures,
            [ObjectFailure {
                path: FACE.path,
                reason: "offline".to_owned()
            }]
        );
    }

    #[test]
    fn a_failed_font_object_drops_the_font_group_and_keeps_the_faces() {
        let layer = IndexLayer::gather([
            (&FONT_JSON, Ok(vec![1])),
            (&FONT_ZIP, Err("offline".to_owned())),
            (&FACE, Ok(vec![2])),
        ]);
        assert_eq!(paths(&layer), [FACE.path]);
        assert_eq!(layer.failures.len(), 1, "only the zip failed");
    }
}
