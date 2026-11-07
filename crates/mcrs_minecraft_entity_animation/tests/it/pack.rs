//! The pig and the frog of the entity pack under `assets/mcrs/entity`.

use std::path::{Path, PathBuf};

use mcrs_minecraft_entity_animation::{ClientEntityDefinition, ClipLibrary, Rig};
use mcrs_minecraft_entity_model::{BakedGeometry, GeometryFile};

pub fn entity_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/entity")
}

pub fn read(path: &str) -> String {
    std::fs::read_to_string(entity_pack().join(path)).unwrap()
}

/// The rig of a mob from its client entity file and its animation file. A diagnostic fails.
pub fn rig(mob: &str) -> Rig {
    let definition =
        ClientEntityDefinition::parse(&read(&format!("entity/{mob}.entity.json"))).unwrap();
    let mut library = ClipLibrary::new();
    library
        .add_file(&read(&format!("animations/{mob}.animation.json")))
        .unwrap();
    let mut diagnostics = Vec::new();
    let rig = Rig::compile(&definition, &library, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{mob}: {diagnostics:#?}");
    rig
}

/// The baked geometries of a geometry file of the pack.
pub fn geometries(file: &str) -> Vec<BakedGeometry> {
    let file: GeometryFile = read(&format!("geometry/{file}.json")).parse().unwrap();
    let mut diagnostics = file.diagnostics.clone();
    file.geometries
        .iter()
        .map(|geometry| geometry.bake(&mut diagnostics).unwrap())
        .collect()
}
