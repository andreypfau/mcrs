use std::collections::HashMap;

use mcrs_minecraft_pack_file::{Json, JsonObject, PackFileError};

/// The fields of one render controller that the client reads.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderControllerSource {
    pub geometry: Option<Json>,
    /// The elements of `textures`; `["Texture.default"]` when the field is absent.
    pub textures: Vec<Json>,
    /// The elements of `part_visibility` in order, each a bone pattern and its value.
    pub part_visibility: Vec<(String, Json)>,
    /// The object `arrays`, which this crate does not compile yet.
    pub has_arrays: bool,
}

/// The render controllers of the render controller files of a pack, by identifier.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderControllerLibrary {
    controllers: HashMap<String, RenderControllerSource>,
}

impl RenderControllerLibrary {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the render controllers of one file. A later file replaces a controller with the
    /// same identifier.
    pub fn add_file(&mut self, text: &str) -> Result<(), PackFileError> {
        let root = Json::parse_lenient(text).map_err(PackFileError::Syntax)?;
        let controllers = root
            .as_object()
            .and_then(|root| root.get("render_controllers"))
            .and_then(Json::as_object)
            .ok_or(PackFileError::NoRoot("render_controllers"))?;
        for (identifier, controller) in controllers.iter() {
            if let Some(controller) = controller.as_object() {
                self.controllers.insert(
                    identifier.to_owned(),
                    RenderControllerSource::read(controller),
                );
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn get(&self, identifier: &str) -> Option<&RenderControllerSource> {
        self.controllers.get(identifier)
    }
}

impl RenderControllerSource {
    fn read(object: &JsonObject) -> Self {
        let textures = match object.get("textures").and_then(Json::as_array) {
            Some(textures) => textures.to_vec(),
            None => vec![Json::String("Texture.default".into())],
        };
        let part_visibility = object
            .get("part_visibility")
            .and_then(Json::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(Json::as_object)
            .flat_map(|element| {
                element
                    .iter()
                    .map(|(pattern, value)| (pattern.to_owned(), value.clone()))
            })
            .collect();
        Self {
            geometry: object.get("geometry").cloned(),
            textures,
            part_visibility,
            has_arrays: object.get("arrays").is_some(),
        }
    }
}
