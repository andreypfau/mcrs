use crate::json::{Json, JsonObject};
use crate::pack_file_error::PackFileError;
use crate::scripts::{AnimateItem, Scripts};

/// A short name of the map `animations` and the clip or controller it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationName {
    /// In lower case.
    pub short_name: String,
    pub identifier: String,
}

/// An element of the `render_controllers` list of a client entity.
#[derive(Clone, Debug, PartialEq)]
pub struct ControllerEntry {
    pub identifier: String,
    /// `None` for an element with no condition.
    pub condition: Option<Json>,
}

/// The description of a client entity file: its resource maps, its animations, its scripts and
/// its render controller list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientEntityFile {
    pub identifier: String,
    /// The short name in lower case and the geometry identifier, in file order.
    pub geometries: Vec<(String, String)>,
    /// The short name in lower case and the texture path, in file order.
    pub textures: Vec<(String, String)>,
    /// The short name in lower case and the material name, in file order.
    pub materials: Vec<(String, String)>,
    /// The map `animations`, then each element of the legacy `animation_controllers` list.
    pub animations: Vec<AnimationName>,
    pub scripts: Scripts,
    pub render_controllers: Vec<ControllerEntry>,
}

impl ClientEntityFile {
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
        let mut file = Self {
            identifier: description
                .get("identifier")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_owned(),
            geometries: short_name_map(description, "geometry"),
            textures: short_name_map(description, "textures"),
            materials: short_name_map(description, "materials"),
            animations: animation_names(description),
            scripts: description
                .get("scripts")
                .and_then(Json::as_object)
                .map(Scripts::read)
                .unwrap_or_default(),
            render_controllers: render_controllers(description),
        };
        file.read_legacy_controllers(description);
        Ok(file)
    }

    /// The identifier that a short name of `animations` names.
    #[must_use]
    pub fn animation(&self, short_name: &str) -> Option<&str> {
        self.animations
            .iter()
            .find(|name| name.short_name.eq_ignore_ascii_case(short_name))
            .map(|name| name.identifier.as_str())
    }

    /// The index of a geometry short name, without regard to letter case.
    #[must_use]
    pub fn geometry(&self, short_name: &str) -> Option<usize> {
        self.geometries
            .iter()
            .position(|(short, _)| short.eq_ignore_ascii_case(short_name))
    }

    /// Each element of the legacy `animation_controllers` list names an animation and plays it
    /// with the weight `1`.
    fn read_legacy_controllers(&mut self, description: &JsonObject) {
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
                self.animations.push(AnimationName {
                    short_name: short_name.clone(),
                    identifier: identifier.into(),
                });
                self.scripts.animate.push(AnimateItem {
                    short_name,
                    weight: None,
                });
            }
        }
    }
}

fn short_name_map(description: &JsonObject, name: &str) -> Vec<(String, String)> {
    description
        .get(name)
        .and_then(Json::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(short, value)| {
                    Some((short.to_ascii_lowercase(), value.as_str()?.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn animation_names(description: &JsonObject) -> Vec<AnimationName> {
    description
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
        .unwrap_or_default()
}

fn render_controllers(description: &JsonObject) -> Vec<ControllerEntry> {
    description
        .get("render_controllers")
        .and_then(Json::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|element| match element {
            Json::String(identifier) => Some(ControllerEntry {
                identifier: identifier.clone(),
                condition: None,
            }),
            Json::Object(object) => {
                let (identifier, condition) = object.iter().next()?;
                Some(ControllerEntry {
                    identifier: identifier.to_owned(),
                    condition: Some(condition.clone()),
                })
            }
            _ => None,
        })
        .collect()
}
