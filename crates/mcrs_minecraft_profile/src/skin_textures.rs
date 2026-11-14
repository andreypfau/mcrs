use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;

use crate::{PlayerModelType, Property};

/// The skin that the `textures` property of a game profile names: the address of its image and
/// the model of its arms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkinTextures {
    pub url: String,
    pub model: PlayerModelType,
}

#[derive(Deserialize)]
struct TexturesValue {
    textures: Textures,
}

#[derive(Deserialize)]
struct Textures {
    #[serde(rename = "SKIN")]
    skin: Option<Skin>,
}

#[derive(Deserialize)]
struct Skin {
    url: String,
    #[serde(default)]
    metadata: Option<SkinMetadata>,
}

#[derive(Deserialize)]
struct SkinMetadata {
    model: Option<String>,
}

impl SkinTextures {
    /// The skin of the first `textures` property of `properties`: its value is base64 of a JSON
    /// object. `None` when no such property names a skin.
    #[must_use]
    pub fn of(properties: &[Property]) -> Option<Self> {
        let property = properties
            .iter()
            .find(|property| property.name == "textures")?;
        let json = STANDARD.decode(&property.value).ok()?;
        let skin = serde_json::from_slice::<TexturesValue>(&json)
            .ok()?
            .textures
            .skin?;
        Some(Self {
            url: skin.url,
            model: match skin.metadata.and_then(|metadata| metadata.model).as_deref() {
                Some("slim") => PlayerModelType::Slim,
                _ => PlayerModelType::Wide,
            },
        })
    }
}
