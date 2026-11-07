use mcrs_minecraft_entity_animation::PackFileError;
use mcrs_minecraft_entity_model::Json;

/// An element of the `render_controllers` list of a client entity.
#[derive(Clone, Debug, PartialEq)]
pub struct ControllerEntry {
    pub identifier: String,
    /// `None` for an element with no condition.
    pub condition: Option<Json>,
}

/// The resource maps and the render controller list of a client entity file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientEntityResources {
    pub identifier: String,
    /// The short name in lower case and the geometry identifier, in file order.
    pub geometries: Vec<(String, String)>,
    /// The short name in lower case and the texture path, in file order.
    pub textures: Vec<(String, String)>,
    /// The short name in lower case and the material name, in file order.
    pub materials: Vec<(String, String)>,
    pub render_controllers: Vec<ControllerEntry>,
}

impl ClientEntityResources {
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
        let map = |name: &str| -> Vec<(String, String)> {
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
        };
        let render_controllers = description
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
            .collect();
        Ok(Self {
            identifier: description
                .get("identifier")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_owned(),
            geometries: map("geometry"),
            textures: map("textures"),
            materials: map("materials"),
            render_controllers,
        })
    }

    /// The index of a geometry short name, without regard to letter case.
    #[must_use]
    pub fn geometry(&self, short_name: &str) -> Option<usize> {
        self.geometries
            .iter()
            .position(|(short, _)| short.eq_ignore_ascii_case(short_name))
    }
}
