use std::collections::HashMap;

use mcrs_minecraft_entity_animation::{ClientEntityDefinition, ClipLibrary, Rig};
use mcrs_minecraft_entity_model::{BakedGeometry, GeometryFile, resolve_parents};
use mcrs_minecraft_render_controller::{
    ClientEntityResources, RenderControllerLibrary, RenderControllers,
};

use crate::entity_library::DefinitionSource;

/// The flip angle of the death tilt when a mob document gives no other, in degrees.
pub const DEATH_FLIP: f32 = 90.0;

/// One client entity of the pack, compiled.
#[derive(Clone, Debug)]
pub struct PackEntity {
    pub resources: ClientEntityResources,
    pub rig: Rig,
    pub controllers: RenderControllers,
    /// The geometries in the order of the geometry map of the client entity.
    pub geometries: Vec<Option<BakedGeometry>>,
    /// The bone patterns of the `part_visibility` of its render controllers.
    pub hideable: Vec<String>,
}

/// The client entities of an entity pack with their animations, geometries and render
/// controllers.
#[derive(Clone, Debug, Default)]
pub struct EntityPack {
    pub entities: Vec<PackEntity>,
    pub diagnostics: Vec<String>,
}

impl EntityPack {
    /// Compiles the files of an entity pack. Each file is its path below the pack root, such as
    /// `entity/pig.entity.json`, and its text.
    #[must_use]
    pub fn load<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut pack = Self::default();
        let mut clips = ClipLibrary::new();
        let mut controllers = RenderControllerLibrary::new();
        let mut geometries: HashMap<String, BakedGeometry> = HashMap::new();
        let mut entities: Vec<(&str, &str)> = Vec::new();
        for (path, text) in files {
            if path.starts_with("animations/") {
                if let Err(error) = clips.add_file(text) {
                    pack.diagnostics.push(format!("{path}: {error}"));
                }
            } else if path.starts_with("render_controllers/") {
                if let Err(error) = controllers.add_file(text) {
                    pack.diagnostics.push(format!("{path}: {error}"));
                }
            } else if path.starts_with("geometry/") && !path.starts_with("geometry/block/") {
                bake_file(path, text, &mut geometries, &mut pack.diagnostics);
            } else if path.starts_with("entity/") {
                entities.push((path, text));
            }
        }
        for (path, text) in entities {
            let (definition, resources) = match (
                ClientEntityDefinition::parse(text),
                ClientEntityResources::parse(text),
            ) {
                (Ok(definition), Ok(resources)) => (definition, resources),
                (Err(error), _) | (_, Err(error)) => {
                    pack.diagnostics.push(format!("{path}: {error}"));
                    continue;
                }
            };
            let mut diagnostics = Vec::new();
            let rig = Rig::compile(&definition, &clips, &mut diagnostics);
            let compiled = RenderControllers::compile(&resources, &controllers, &mut diagnostics);
            pack.diagnostics.extend(
                diagnostics
                    .iter()
                    .map(|diagnostic| format!("{path}: {diagnostic}")),
            );
            let geometries = resources
                .geometries
                .iter()
                .map(|(short, identifier)| {
                    let found = geometries.get(identifier).cloned();
                    if found.is_none() {
                        pack.diagnostics.push(format!(
                            "{path}: geometry {short} names no geometry {identifier}"
                        ));
                    }
                    found
                })
                .collect();
            let hideable = compiled
                .layers
                .iter()
                .flat_map(|layer| {
                    layer
                        .part_visibility
                        .iter()
                        .map(|(pattern, _)| pattern.clone())
                })
                .collect();
            pack.entities.push(PackEntity {
                resources,
                rig,
                controllers: compiled,
                geometries,
                hideable,
            });
        }
        pack
    }

    /// The client entity with the identifier `identifier`.
    #[must_use]
    pub fn entity(&self, identifier: &str) -> Option<(usize, &PackEntity)> {
        self.entities
            .iter()
            .enumerate()
            .find(|(_, entity)| entity.resources.identifier == identifier)
    }

    /// The definitions of the entity library, in the order of [`EntityPack::entities`].
    #[must_use]
    pub fn sources(&self) -> Vec<DefinitionSource<'_>> {
        self.entities
            .iter()
            .map(|entity| DefinitionSource {
                rig: &entity.rig,
                geometries: &entity.geometries,
                hideable: &entity.hideable,
                death_flip: DEATH_FLIP,
            })
            .collect()
    }
}

/// Bakes each geometry of a geometry file into `geometries`, by identifier.
fn bake_file(
    path: &str,
    text: &str,
    geometries: &mut HashMap<String, BakedGeometry>,
    diagnostics: &mut Vec<String>,
) {
    let file = match text.parse::<GeometryFile>() {
        Ok(file) => file,
        Err(error) => {
            diagnostics.push(format!("{path}: {error}"));
            return;
        }
    };
    let mut found = file.diagnostics;
    let resolved = resolve_parents(file.geometries, &mut found);
    for geometry in resolved {
        match geometry.bake(&mut found) {
            Ok(baked) => {
                geometries.insert(baked.identifier.clone(), baked);
            }
            Err(error) => diagnostics.push(format!("{path}: {error}")),
        }
    }
    diagnostics.extend(
        found
            .iter()
            .map(|diagnostic| format!("{path}: {diagnostic}")),
    );
}
