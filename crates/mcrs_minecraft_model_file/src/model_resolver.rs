use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::block_state_file::MISSING_MODEL;
use crate::content_error::{ContentError, ContentErrorKind};
use crate::model_file::{
    DisplayContext, DisplayTransform, Element, GuiLight, ModelFile, SlotValue, TextureValue,
};
use crate::pack_files::PackFiles;
use crate::resource_id::{full_id, resource_path};

pub const GENERATED_MODEL: &str = "minecraft:builtin/generated";
pub const MISSING_TEXTURE: &str = "minecraft:missingno";

const MISSING_MODEL_FILE: &str = r##"{
  "textures": { "particle": "#missingno", "missingno": "minecraft:missingno" },
  "elements": [
    {
      "from": [0, 0, 0],
      "to": [16, 16, 16],
      "faces": {
        "down":  { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "down" },
        "up":    { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "up" },
        "north": { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "north" },
        "south": { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "south" },
        "west":  { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "west" },
        "east":  { "uv": [0, 0, 16, 16], "texture": "#missingno", "cullface": "east" }
      }
    }
  ]
}"##;

const GENERATED_MODEL_FILE: &str =
    r##"{ "textures": { "particle": "#layer0" }, "gui_light": "front" }"##;

/// A model with its chain merged: the fields it inherits and its resolved texture slots.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MergedModel {
    /// The model of the chain that supplies the elements.
    pub geometry_from: Option<String>,
    pub elements: Vec<Element>,
    /// The elements are the extruded layers of `minecraft:builtin/generated`.
    pub generated: bool,
    pub ambient_occlusion: bool,
    pub gui_light: GuiLight,
    pub textures: BTreeMap<String, TextureValue>,
    /// The slots of the chain that resolve to no texture.
    pub unresolved: BTreeSet<String>,
    pub display: [Option<DisplayTransform>; 9],
}

impl MergedModel {
    /// The texture of a face whose `texture` string is `reference`, with or without its `#`.
    #[must_use]
    pub fn texture(&self, reference: &str) -> Option<&TextureValue> {
        self.textures
            .get(reference.strip_prefix('#').unwrap_or(reference))
    }

    /// The sprite a face with this `texture` string draws: the missing texture when its slot
    /// has none.
    #[must_use]
    pub fn sprite(&self, reference: &str) -> TextureValue {
        self.texture(reference).cloned().unwrap_or(TextureValue {
            sprite: MISSING_TEXTURE.to_owned(),
            force_translucent: false,
        })
    }

    #[must_use]
    pub fn particle(&self) -> TextureValue {
        self.sprite("particle")
    }

    #[must_use]
    pub fn display(&self, context: DisplayContext) -> Option<DisplayTransform> {
        self.display.get(context.index()).copied().flatten()
    }
}

/// Reads model files from the files of a pack and merges their chains, each model once.
pub struct ModelResolver<'a> {
    pack: &'a PackFiles,
    files: HashMap<String, Option<ModelFile>>,
    merged: HashMap<String, MergedModel>,
    errors: Vec<ContentError>,
}

impl<'a> ModelResolver<'a> {
    #[must_use]
    pub fn new(pack: &'a PackFiles) -> Self {
        let mut files = HashMap::new();
        for (id, text) in [
            (MISSING_MODEL, MISSING_MODEL_FILE),
            (GENERATED_MODEL, GENERATED_MODEL_FILE),
        ] {
            files.insert(id.to_owned(), ModelFile::parse(text.as_bytes()).ok());
        }
        Self {
            pack,
            files,
            merged: HashMap::new(),
            errors: Vec::new(),
        }
    }

    #[must_use]
    pub fn pack(&self) -> &'a PackFiles {
        self.pack
    }

    /// The content errors found so far, each naming its file.
    #[must_use]
    pub fn errors(&self) -> &[ContentError] {
        &self.errors
    }

    pub fn take_errors(&mut self) -> Vec<ContentError> {
        std::mem::take(&mut self.errors)
    }

    /// The model `id` with its chain merged. A model that is absent, defective or in a cycle
    /// gives the missing model.
    pub fn resolve(&mut self, id: &str) -> &MergedModel {
        let id = full_id(id);
        if !self.merged.contains_key(&id) {
            let chain = self
                .chain(&id)
                .unwrap_or_else(|| vec![MISSING_MODEL.to_owned()]);
            let merged = self.merge(&chain);
            self.merged.insert(id.clone(), merged);
        }
        self.merged.entry(id).or_default()
    }

    fn load(&mut self, id: &str) -> bool {
        if let Some(file) = self.files.get(id) {
            return file.is_some();
        }
        let path = resource_path(id, "models", "json");
        let file = match self.pack.get(&path) {
            None => {
                self.errors.push(ContentError {
                    path,
                    kind: ContentErrorKind::NoModel,
                });
                None
            }
            Some(bytes) => match ModelFile::parse(bytes) {
                Ok(file) => Some(file),
                Err(error) => {
                    self.errors.push(ContentError {
                        path,
                        kind: ContentErrorKind::Model(error),
                    });
                    None
                }
            },
        };
        let present = file.is_some();
        self.files.insert(id.to_owned(), file);
        present
    }

    /// The identifiers of the chain of `id`, or `None` when `id` itself is absent or in a cycle.
    fn chain(&mut self, id: &str) -> Option<Vec<String>> {
        if !self.load(id) {
            return None;
        }
        let mut chain = vec![id.to_owned()];
        let mut at = id.to_owned();
        loop {
            let parent = self
                .files
                .get(&at)
                .and_then(Option::as_ref)
                .and_then(|file| file.parent.clone());
            let Some(parent) = parent else {
                return Some(chain);
            };
            if chain.contains(&parent) {
                self.errors.push(ContentError {
                    path: id.to_owned(),
                    kind: ContentErrorKind::ModelCycle { parent },
                });
                return None;
            }
            let parent = if self.load(&parent) {
                parent
            } else {
                MISSING_MODEL.to_owned()
            };
            chain.push(parent.clone());
            at = parent;
        }
    }

    fn merge(&self, chain: &[String]) -> MergedModel {
        let files: Vec<(&str, &ModelFile)> = chain
            .iter()
            .filter_map(|id| Some((id.as_str(), self.files.get(id)?.as_ref()?)))
            .collect();
        let geometry = files
            .iter()
            .find(|(id, file)| file.elements.is_some() || *id == GENERATED_MODEL);
        let mut slots: BTreeMap<&str, &SlotValue> = BTreeMap::new();
        for (_, file) in files.iter().rev() {
            for (slot, value) in &file.textures {
                slots.insert(slot, value);
            }
        }
        let display = std::array::from_fn(|index| {
            files
                .iter()
                .find_map(|(_, file)| file.display.get(index).copied().flatten())
        });
        let (textures, unresolved) = resolve_slots(&slots);
        MergedModel {
            geometry_from: geometry.map(|(id, _)| (*id).to_owned()),
            elements: geometry
                .and_then(|(_, file)| file.elements.clone())
                .unwrap_or_default(),
            generated: geometry.is_some_and(|(id, _)| *id == GENERATED_MODEL),
            ambient_occlusion: files
                .iter()
                .find_map(|(_, file)| file.ambient_occlusion)
                .unwrap_or(true),
            gui_light: files
                .iter()
                .find_map(|(_, file)| file.gui_light)
                .unwrap_or_default(),
            textures,
            unresolved,
            display,
        }
    }
}

fn resolve_slots(
    slots: &BTreeMap<&str, &SlotValue>,
) -> (BTreeMap<String, TextureValue>, BTreeSet<String>) {
    let mut resolved: BTreeMap<String, TextureValue> = slots
        .iter()
        .filter_map(|(slot, value)| match value {
            SlotValue::Texture(texture) => Some(((*slot).to_owned(), texture.clone())),
            SlotValue::Reference(_) => None,
        })
        .collect();
    loop {
        let mut changed = false;
        for (slot, value) in slots {
            let SlotValue::Reference(target) = value else {
                continue;
            };
            if resolved.contains_key(*slot) {
                continue;
            }
            if let Some(texture) = resolved.get(target).cloned() {
                resolved.insert((*slot).to_owned(), texture);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let unresolved = slots
        .keys()
        .filter(|slot| !resolved.contains_key(**slot))
        .map(|slot| (*slot).to_owned())
        .collect();
    (resolved, unresolved)
}
