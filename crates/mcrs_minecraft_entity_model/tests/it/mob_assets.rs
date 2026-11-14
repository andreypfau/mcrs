//! Writes the geometry files of the mobs from the model artefact, and checks that the files in
//! the repository are current.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glam::Vec3;
use mcrs_minecraft_entity_model::{BakedGeometry, BonePose, GeometryFile};
use mcrs_minecraft_fixtures::fixture;

use crate::artefact::ModelArtefact;
use crate::convert::{self, Bone, Geometry, Rules};

const GEOMETRY_DIRECTORY: &str = "mcrs/entity/geometry";

struct Source {
    identifier: &'static str,
    model: &'static str,
    layer: &'static str,
    skeleton: Option<&'static Skeleton>,
}

const PIG: [Source; 4] = [
    Source {
        identifier: "geometry.pig",
        model: "minecraft:pig",
        layer: "main",
        skeleton: None,
    },
    Source {
        identifier: "geometry.pig.cold",
        model: "minecraft:cold_pig",
        layer: "main",
        skeleton: None,
    },
    Source {
        identifier: "geometry.pig.baby",
        model: "minecraft:pig_baby",
        layer: "main",
        skeleton: None,
    },
    Source {
        identifier: "geometry.pig.saddle",
        model: "minecraft:pig",
        layer: "saddle",
        skeleton: None,
    },
];

const FROG: [Source; 1] = [Source {
    identifier: "geometry.frog",
    model: "minecraft:frog",
    layer: "main",
    skeleton: None,
}];

const PLAYER: [Source; 2] = [
    Source {
        identifier: "geometry.player",
        model: "minecraft:player",
        layer: "main",
        skeleton: Some(&WIDE_SKELETON),
    },
    Source {
        identifier: "geometry.player.slim",
        model: "minecraft:player_slim",
        layer: "main",
        skeleton: Some(&SLIM_SKELETON),
    },
];

/// A bone with no box that the player model adds to the bones of the artefact.
struct AddedBone {
    name: &'static str,
    parent: Option<&'static str>,
    pivot: [f64; 3],
}

/// The parent that a root bone of the artefact gets.
struct Reparent {
    bone: &'static str,
    parent: &'static str,
}

/// The bones and the parents of a player model that the artefact does not give.
struct Skeleton {
    /// Placed before the bones of the artefact.
    frame: [AddedBone; 2],
    reparents: [Reparent; 6],
    /// Placed after the bones of the artefact.
    attachments: [AddedBone; 4],
}

const fn player_skeleton(item_x: f64) -> Skeleton {
    Skeleton {
        frame: [
            AddedBone {
                name: "root",
                parent: None,
                pivot: [0.0, 0.0, 0.0],
            },
            AddedBone {
                name: "waist",
                parent: Some("root"),
                pivot: [0.0, 12.0, 0.0],
            },
        ],
        reparents: [
            Reparent {
                bone: "body",
                parent: "waist",
            },
            Reparent {
                bone: "head",
                parent: "waist",
            },
            Reparent {
                bone: "right_arm",
                parent: "waist",
            },
            Reparent {
                bone: "left_arm",
                parent: "waist",
            },
            Reparent {
                bone: "right_leg",
                parent: "root",
            },
            Reparent {
                bone: "left_leg",
                parent: "root",
            },
        ],
        attachments: [
            AddedBone {
                name: "cape",
                parent: Some("waist"),
                pivot: [0.0, 24.0, 2.0],
            },
            AddedBone {
                name: "elytra",
                parent: Some("waist"),
                pivot: [0.0, 24.0, 2.0],
            },
            AddedBone {
                name: "right_item",
                parent: Some("right_arm"),
                pivot: [-item_x, 12.0, -2.0],
            },
            AddedBone {
                name: "left_item",
                parent: Some("left_arm"),
                pivot: [item_x, 12.0, -2.0],
            },
        ],
    }
}

const WIDE_SKELETON: Skeleton = player_skeleton(6.0);
const SLIM_SKELETON: Skeleton = player_skeleton(5.5);

fn added_bone(bone: &AddedBone) -> Bone {
    Bone {
        name: bone.name.to_owned(),
        parent: bone.parent.map(str::to_owned),
        pivot: bone.pivot,
        rotation: [0.0; 3],
        cubes: Vec::new(),
    }
}

fn apply_skeleton(geometry: &mut Geometry, skeleton: &Skeleton) {
    for reparent in &skeleton.reparents {
        let bone = geometry
            .bones
            .iter_mut()
            .find(|bone| bone.name == reparent.bone)
            .unwrap_or_else(|| panic!("the artefact has no bone {}", reparent.bone));
        assert!(
            bone.parent.is_none(),
            "a reparented bone is a root bone of the artefact: {}",
            bone.name
        );
        bone.parent = Some(reparent.parent.to_owned());
    }
    let mut bones: Vec<Bone> = skeleton.frame.iter().map(added_bone).collect();
    bones.append(&mut geometry.bones);
    bones.extend(skeleton.attachments.iter().map(added_bone));
    geometry.bones = bones;
}

fn geometry_file(artefact: &ModelArtefact, sources: &[Source]) -> String {
    let geometries = sources
        .iter()
        .map(|source| {
            let entry = artefact.layer_entry(source.model, source.layer);
            let mut geometry = convert::convert(entry, source.identifier, Rules::Mob)
                .unwrap_or_else(|refusal| panic!("{}: {refusal}", source.identifier));
            if let Some(skeleton) = source.skeleton {
                apply_skeleton(&mut geometry, skeleton);
            }
            geometry
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

fn cube_corner(baked: &BakedGeometry, bone: &str) -> Vec3 {
    let bone = baked.bone_index(bone).unwrap();
    let cube = baked.cubes.iter().find(|cube| cube.bone == bone).unwrap();
    cube.shape.origin
}

#[test]
fn a_turn_of_the_waist_turns_the_upper_body_about_the_waist_and_leaves_the_legs() {
    let text =
        std::fs::read_to_string(assets().join(GEOMETRY_DIRECTORY).join("player.json")).unwrap();
    let file: GeometryFile = text.parse().unwrap();
    for geometry in &file.geometries {
        let baked = geometry.bake(&mut Vec::new()).unwrap();
        let mut poses: Vec<BonePose> = baked.bones.iter().map(BonePose::rest).collect();
        poses[baked.bone_index("waist").unwrap()].rotation = Vec3::new(30.0, 0.0, 0.0);
        let transforms = baked.model_transforms(&poses);
        let waist_pivot = Vec3::new(0.0, 12.0, 0.0);
        let turned_pivot =
            transforms[baked.bone_index("body").unwrap()].transform_point3(waist_pivot);
        assert!(
            turned_pivot.distance(waist_pivot) < 1e-3,
            "{}: the waist turns about {waist_pivot}, not {turned_pivot}",
            baked.identifier
        );
        for (bone, moves) in [
            ("head", true),
            ("body", true),
            ("right_arm", true),
            ("left_arm", true),
            ("left_leg", false),
            ("right_leg", false),
        ] {
            let corner = cube_corner(&baked, bone);
            let posed = transforms[baked.bone_index(bone).unwrap()].transform_point3(corner);
            assert_eq!(
                posed.distance(corner) > 1e-3,
                moves,
                "{}: a turn of the waist moves {bone} from {corner} to {posed}",
                baked.identifier
            );
        }
    }
}
