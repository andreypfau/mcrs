use mcrs_minecraft_entity_model::{Json, JsonObject};

use crate::pack_file_error::PackFileError;

/// A short name of the map `animations` and the clip or controller it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationName {
    /// In lower case.
    pub short_name: String,
    pub identifier: String,
}

/// An element of the `animate` list.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimateItem {
    /// In lower case.
    pub short_name: String,
    /// `None` for the weight `1`.
    pub weight: Option<Json>,
}

/// The fields of `scripts` that the animation host reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scripts {
    pub initialize: Vec<String>,
    pub pre_animation: Vec<String>,
    pub animate: Vec<AnimateItem>,
    /// `scale`, `scalex`, `scaley` and `scalez`.
    pub scale: [Option<Json>; 4],
}

/// The animation part of a client entity file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientEntityDefinition {
    pub identifier: String,
    pub animations: Vec<AnimationName>,
    pub scripts: Scripts,
}

impl ClientEntityDefinition {
    /// Reads a client entity file. A field with a wrong shape reads as absent.
    pub fn parse(text: &str) -> Result<Self, PackFileError> {
        let root = Json::parse_lenient(text).map_err(PackFileError::Syntax)?;
        let description = root
            .as_object()
            .and_then(|root| root.get("minecraft:client_entity"))
            .and_then(Json::as_object)
            .and_then(|entity| entity.get("description"))
            .and_then(Json::as_object)
            .ok_or(PackFileError::NoRoot("minecraft:client_entity.description"))?;
        let identifier = description
            .get("identifier")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .into();
        let mut animations: Vec<AnimationName> = description
            .get("animations")
            .and_then(Json::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(short_name, identifier)| {
                        Some(AnimationName {
                            short_name: short_name.to_ascii_lowercase(),
                            identifier: identifier.as_str()?.into(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut scripts = description
            .get("scripts")
            .and_then(Json::as_object)
            .map(Scripts::read)
            .unwrap_or_default();
        let legacy = description
            .get("animation_controllers")
            .and_then(Json::as_array)
            .unwrap_or_default();
        for element in legacy.iter().filter_map(Json::as_object) {
            for (short_name, identifier) in element.iter() {
                let Some(identifier) = identifier.as_str() else {
                    continue;
                };
                let short_name = short_name.to_ascii_lowercase();
                animations.push(AnimationName {
                    short_name: short_name.clone(),
                    identifier: identifier.into(),
                });
                scripts.animate.push(AnimateItem {
                    short_name,
                    weight: None,
                });
            }
        }
        Ok(Self {
            identifier,
            animations,
            scripts,
        })
    }

    /// The identifier that a short name of `animations` names.
    #[must_use]
    pub fn animation(&self, short_name: &str) -> Option<&str> {
        self.animations
            .iter()
            .find(|name| name.short_name.eq_ignore_ascii_case(short_name))
            .map(|name| name.identifier.as_str())
    }
}

impl Scripts {
    fn read(object: &JsonObject) -> Self {
        let field = |name: &str| {
            object
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        };
        let strings = |name: &str| -> Vec<String> {
            match field(name) {
                Some(Json::Array(entries)) => entries
                    .iter()
                    .filter_map(Json::as_str)
                    .map(String::from)
                    .collect(),
                Some(Json::String(text)) => vec![text.clone()],
                _ => Vec::new(),
            }
        };
        let animate = field("animate")
            .and_then(Json::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|element| match element {
                Json::String(short_name) => Some(AnimateItem {
                    short_name: short_name.to_ascii_lowercase(),
                    weight: None,
                }),
                Json::Object(object) => {
                    let (short_name, weight) = object.iter().next()?;
                    Some(AnimateItem {
                        short_name: short_name.to_ascii_lowercase(),
                        weight: Some(weight.clone()),
                    })
                }
                _ => None,
            })
            .collect();
        Self {
            initialize: strings("initialize"),
            pre_animation: strings("pre_animation"),
            animate,
            scale: ["scale", "scalex", "scaley", "scalez"].map(|name| field(name).cloned()),
        }
    }
}
