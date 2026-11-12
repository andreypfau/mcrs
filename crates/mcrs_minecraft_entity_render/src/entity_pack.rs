use std::collections::HashMap;

use mcrs_minecraft_entity_animation::{ClipLibrary, Rig};
use mcrs_minecraft_entity_model::{
    BakedGeometry, Diagnostic, Geometry, GeometryFile, resolve_parents,
};
use mcrs_minecraft_pack_file::ClientEntityFile;
use mcrs_minecraft_render_controller::{RenderControllerLibrary, RenderControllers};

use crate::entity_library::DefinitionSource;
use crate::repository_pack::REPOSITORY_PACK;

/// The flip angle of the death tilt when a mob document gives no other, in degrees.
pub const DEATH_FLIP: f32 = 90.0;

/// One client entity of the pack, compiled.
#[derive(Clone, Debug)]
pub struct PackEntity {
    pub resources: ClientEntityFile,
    pub rig: Rig,
    pub controllers: RenderControllers,
    /// The geometries in the order of the geometry map of the client entity.
    pub geometries: Vec<Option<BakedGeometry>>,
}

/// The client entities of an entity pack with their animations, geometries and render
/// controllers.
#[derive(Clone, Debug, Default)]
pub struct EntityPack {
    pub entities: Vec<PackEntity>,
    pub diagnostics: Vec<String>,
}

impl EntityPack {
    /// The entity pack of the repository, `assets/mcrs/entity`, which the crate embeds.
    #[must_use]
    pub fn repository() -> Self {
        Self::load(REPOSITORY_PACK)
    }

    /// Compiles the files of an entity pack. Each file is its path below the pack root, such as
    /// `entity/pig.entity.json`, and its text.
    #[must_use]
    pub fn load<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut pack = Self::default();
        let mut clips = ClipLibrary::new();
        let mut controllers = RenderControllerLibrary::new();
        let mut read: Vec<Geometry> = Vec::new();
        let mut geometry_paths: HashMap<String, &str> = HashMap::new();
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
                read_geometry_file(
                    path,
                    text,
                    &mut read,
                    &mut geometry_paths,
                    &mut pack.diagnostics,
                );
            } else if path.starts_with("entity/") {
                entities.push((path, text));
            }
        }
        let geometries = bake_geometries(read, &geometry_paths, &mut pack.diagnostics);
        for (path, text) in entities {
            let resources = match ClientEntityFile::parse(text) {
                Ok(resources) => resources,
                Err(error) => {
                    pack.diagnostics.push(format!("{path}: {error}"));
                    continue;
                }
            };
            let mut diagnostics = Vec::new();
            let rig = Rig::compile(&resources, &clips, &mut diagnostics);
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
            pack.entities.push(PackEntity {
                resources,
                rig,
                controllers: compiled,
                geometries,
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
                controllers: &entity.controllers,
                death_flip: DEATH_FLIP,
            })
            .collect()
    }
}

/// Reads the geometries of a geometry file into `geometries`. `paths` takes the path of each
/// geometry identifier.
fn read_geometry_file<'a>(
    path: &'a str,
    text: &str,
    geometries: &mut Vec<Geometry>,
    paths: &mut HashMap<String, &'a str>,
    diagnostics: &mut Vec<String>,
) {
    let file = match text.parse::<GeometryFile>() {
        Ok(file) => file,
        Err(error) => {
            diagnostics.push(format!("{path}: {error}"));
            return;
        }
    };
    diagnostics.extend(
        file.diagnostics
            .iter()
            .map(|diagnostic| format!("{path}: {diagnostic}")),
    );
    for geometry in file.geometries {
        paths.insert(geometry.identifier.clone(), path);
        geometries.push(geometry);
    }
}

/// Resolves the legacy parents across all geometry files, then bakes each geometry, by
/// identifier. A legacy geometry can name a parent of another file.
fn bake_geometries(
    read: Vec<Geometry>,
    paths: &HashMap<String, &str>,
    diagnostics: &mut Vec<String>,
) -> HashMap<String, BakedGeometry> {
    let path_of = |identifier: &str| paths.get(identifier).copied().unwrap_or("geometry");
    let mut rejected = Vec::new();
    let resolved = resolve_parents(read, &mut rejected);
    for diagnostic in &rejected {
        let path = match diagnostic {
            Diagnostic::RejectedGeometry {
                identifier: Some(identifier),
                ..
            } => path_of(identifier),
            _ => "geometry",
        };
        diagnostics.push(format!("{path}: {diagnostic}"));
    }
    let mut geometries = HashMap::new();
    for geometry in resolved {
        let path = path_of(&geometry.identifier);
        let mut found = Vec::new();
        match geometry.bake(&mut found) {
            Ok(baked) => {
                geometries.insert(baked.identifier.clone(), baked);
            }
            Err(error) => diagnostics.push(format!("{path}: {error}")),
        }
        diagnostics.extend(
            found
                .iter()
                .map(|diagnostic| format!("{path}: {diagnostic}")),
        );
    }
    geometries
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARENT: &str = r#"{"format_version": "1.8.0", "geometry.parent": {
        "texturewidth": 16, "textureheight": 16,
        "bones": [{"name": "body", "pivot": [0, 0, 0], "cubes": [{"origin": [0, 0, 0], "size": [1, 1, 1], "uv": [0, 0]}]}]}}"#;
    const CHILD: &str = r#"{"format_version": "1.8.0", "geometry.child:geometry.parent": {
        "bones": [{"name": "head", "parent": "body", "pivot": [0, 1, 0], "cubes": [{"origin": [0, 1, 0], "size": [1, 1, 1], "uv": [0, 0]}]}]}}"#;

    #[test]
    fn the_repository_pack_draws_arrows_and_experience_orbs() {
        let pack = EntityPack::repository();
        assert!(pack.diagnostics.is_empty(), "{:?}", pack.diagnostics);
        assert!(pack.entity("minecraft:arrow").is_some());
        assert!(pack.entity("minecraft:experience_orb").is_some());
    }

    #[test]
    fn a_legacy_geometry_resolves_a_parent_of_another_file() {
        let pack = EntityPack::load([
            ("geometry/child.geo.json", CHILD),
            ("geometry/parent.geo.json", PARENT),
        ]);
        assert!(pack.diagnostics.is_empty(), "{:?}", pack.diagnostics);
        let mut geometries = Vec::new();
        let mut paths = HashMap::new();
        let mut diagnostics = Vec::new();
        for (path, text) in [("child", CHILD), ("parent", PARENT)] {
            read_geometry_file(path, text, &mut geometries, &mut paths, &mut diagnostics);
        }
        let baked = bake_geometries(geometries, &paths, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let child = &baked["geometry.child"];
        let names: Vec<&str> = child.bones.iter().map(|bone| bone.name.as_str()).collect();
        assert_eq!(names, ["body", "head"]);
        assert_eq!(child.bones[1].parent, Some(0));
    }
}
