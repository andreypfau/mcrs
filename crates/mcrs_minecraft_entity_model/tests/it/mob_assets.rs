//! Writes the geometry files of the mobs from the model artefact, and checks that the files in
//! the repository are current.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mcrs_minecraft_entity_model::GeometryFile;
use mcrs_minecraft_fixtures::fixture;

use crate::artefact::ModelArtefact;
use crate::convert::{self, Rules};

const GEOMETRY_DIRECTORY: &str = "mcrs/entity/geometry";

struct Source {
    identifier: &'static str,
    model: &'static str,
    layer: &'static str,
}

const PIG: [Source; 4] = [
    Source {
        identifier: "geometry.pig",
        model: "minecraft:pig",
        layer: "main",
    },
    Source {
        identifier: "geometry.pig.cold",
        model: "minecraft:cold_pig",
        layer: "main",
    },
    Source {
        identifier: "geometry.pig.baby",
        model: "minecraft:pig_baby",
        layer: "main",
    },
    Source {
        identifier: "geometry.pig.saddle",
        model: "minecraft:pig",
        layer: "saddle",
    },
];

const FROG: [Source; 1] = [Source {
    identifier: "geometry.frog",
    model: "minecraft:frog",
    layer: "main",
}];

const PLAYER: [Source; 2] = [
    Source {
        identifier: "geometry.player",
        model: "minecraft:player",
        layer: "main",
    },
    Source {
        identifier: "geometry.player.slim",
        model: "minecraft:player_slim",
        layer: "main",
    },
];

fn geometry_file(artefact: &ModelArtefact, sources: &[Source]) -> String {
    let geometries = sources
        .iter()
        .map(|source| {
            let entry = artefact.layer_entry(source.model, source.layer);
            convert::convert(entry, source.identifier, Rules::Mob)
                .unwrap_or_else(|refusal| panic!("{}: {refusal}", source.identifier))
        })
        .collect();
    convert::to_json(&convert::GeometryFile {
        format_version: "1.12.0",
        geometries,
    })
}

/// Each generated file, keyed by its path below `assets/`.
fn generate() -> BTreeMap<PathBuf, String> {
    let artefact = ModelArtefact::parse(&fixture("entity_models").text());
    let directory = Path::new(GEOMETRY_DIRECTORY);
    BTreeMap::from([
        (directory.join("pig.json"), geometry_file(&artefact, &PIG)),
        (directory.join("frog.json"), geometry_file(&artefact, &FROG)),
        (
            directory.join("player.json"),
            geometry_file(&artefact, &PLAYER),
        ),
    ])
}

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// The geometry files directly in the geometry directory; the subdirectories have other
/// generators.
fn checked_in() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(assets().join(GEOMETRY_DIRECTORY)) else {
        return Vec::new();
    };
    entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| Path::new(GEOMETRY_DIRECTORY).join(path.file_name().unwrap()))
        .collect()
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a developer asks for the files to be written again through the environment"
)]
fn the_mob_geometry_files_are_generated_from_the_artefact() {
    let generated = generate();
    for (path, text) in &generated {
        let file: GeometryFile = text.parse().unwrap();
        let mut diagnostics = file.diagnostics.clone();
        for geometry in &file.geometries {
            geometry.bake(&mut diagnostics).unwrap();
        }
        assert!(
            diagnostics.is_empty(),
            "{} has diagnostics: {diagnostics:?}",
            path.display()
        );
    }
    let stale: Vec<PathBuf> = checked_in()
        .into_iter()
        .filter(|path| !generated.contains_key(path))
        .collect();
    if std::env::var("MCRS_BLESS").is_ok() {
        for path in &stale {
            std::fs::remove_file(assets().join(path)).unwrap();
        }
        for (path, text) in &generated {
            std::fs::write(assets().join(path), text).unwrap();
        }
        return;
    }
    assert!(stale.is_empty(), "files that no source writes: {stale:?}");
    for (path, text) in &generated {
        let held = std::fs::read_to_string(assets().join(path)).unwrap_or_default();
        assert!(
            held == *text,
            "assets/{} is stale; rerun with MCRS_BLESS=1 to write it again",
            path.display()
        );
    }
}
