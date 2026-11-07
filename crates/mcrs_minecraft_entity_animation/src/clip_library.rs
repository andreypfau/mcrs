use std::collections::HashMap;

use mcrs_minecraft_entity_model::{Json, JsonObject};

use crate::pack_file_error::PackFileError;

/// The clips of the animation files of a pack, by identifier, before a client entity compiles
/// them.
#[derive(Clone, Debug, Default)]
pub struct ClipLibrary {
    clips: HashMap<String, JsonObject>,
}

impl ClipLibrary {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the clips of an animation file. A clip replaces an earlier clip with the same
    /// identifier. A clip that is not an object is left out.
    pub fn add_file(&mut self, text: &str) -> Result<(), PackFileError> {
        let root = Json::parse_lenient(text).map_err(PackFileError::Syntax)?;
        let animations = root
            .as_object()
            .and_then(|root| root.get("animations"))
            .and_then(Json::as_object)
            .ok_or(PackFileError::NoRoot("animations"))?;
        for (identifier, clip) in animations.iter() {
            if let Some(clip) = clip.as_object() {
                self.clips.insert(identifier.into(), clip.clone());
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn get(&self, identifier: &str) -> Option<&JsonObject> {
        self.clips.get(identifier)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.clips.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }
}
