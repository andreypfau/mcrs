//! Writes the geometry files and the data files of the static block entities from the oracle
//! artefacts and the shape tables, and checks that the files in the repository are current.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::artefact::{Artefacts, Cube, LayerRecord, Part, PartTransform};
use crate::block_entity_shapes::{self, BlockShapes, GeometrySource};

const GEOMETRY_DIRECTORY: &str = "mcrs/entity/geometry/block";
const DATA_DIRECTORY: &str = "mcrs/block_entity_model/minecraft";
const SHAPE_ORIGIN_HEIGHT: f64 = 24.0;

#[derive(Serialize)]
struct GeometryFile {
    format_version: &'static str,
    #[serde(rename = "minecraft:geometry")]
    geometries: Vec<Geometry>,
}

#[derive(Serialize)]
struct Geometry {
    description: Description,
    bones: Vec<Bone>,
}

#[derive(Serialize)]
struct Description {
    identifier: String,
    texture_width: f64,
    texture_height: f64,
}

#[derive(Serialize)]
struct Bone {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    pivot: [f64; 3],
    #[serde(skip_serializing_if = "is_zero")]
    rotation: [f64; 3],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cubes: Vec<GeometryCube>,
}

#[derive(Serialize)]
struct GeometryCube {
    origin: [f64; 3],
    size: [f64; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    inflate: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    mirror: bool,
    uv: CubeUv,
}

#[derive(Serialize)]
#[serde(untagged)]
enum CubeUv {
    Box([f64; 2]),
    ByFace(BTreeMap<&'static str, FaceUv>),
}

#[derive(Serialize)]
struct FaceUv {
    uv: [f64; 2],
    uv_size: [f64; 2],
}

#[derive(Serialize)]
struct DataFile {
    layers: Vec<DataLayer>,
}

#[derive(Serialize)]
struct DataLayer {
    #[serde(skip_serializing_if = "Option::is_none")]
    when: Option<String>,
    geometry: String,
    texture: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tint: Option<u8>,
    surface: &'static str,
    back_faces: bool,
    placement: [[f64; 4]; 3],
}

fn is_zero(values: &[f64; 3]) -> bool {
    values.iter().all(|value| *value == 0.0)
}

fn round(value: f64, step: f64) -> f64 {
    (value / step).round() * step + 0.0
}

fn model_units(value: f64) -> f64 {
    round(value, 1e-6)
}

/// The box UV rectangle `[u1, v1, u2, v2]` of one face named in internal space. The
/// reference lays out the box from the exact size, which differs from the floored layout of
/// the geometry format when a size has a fraction.
fn box_rect(face: &str, [u, v]: [f64; 2], size: [f64; 3], mirror: bool) -> [f64; 4] {
    let [w, h, d] = size;
    let face = match (mirror, face) {
        (true, "east") => "west",
        (true, "west") => "east",
        (_, face) => face,
    };
    let [u1, v1, u2, v2] = match face {
        "east" => [u, v + d, u + d, v + d + h],
        "north" => [u + d, v + d, u + d + w, v + d + h],
        "west" => [u + d + w, v + d, u + 2.0 * d + w, v + d + h],
        "south" => [u + 2.0 * d + w, v + d, u + 2.0 * d + 2.0 * w, v + d + h],
        "up" => [u + d + w, v + d, u + d, v],
        "down" => [u + d + 2.0 * w, v, u + d + w, v + d],
        other => panic!("{other} is not a face"),
    };
    if mirror {
        [u2, v1, u1, v2]
    } else {
        [u1, v1, u2, v2]
    }
}

fn face_name(name: &str) -> &'static str {
    match name {
        "east" => "east",
        "north" => "north",
        "west" => "west",
        "south" => "south",
        "up" => "up",
        "down" => "down",
        other => panic!("{other} is not a face"),
    }
}

/// The per-face form of the box UV of the faces of `cube` that the artefact lists.
fn per_face_uv(cube: &Cube) -> BTreeMap<&'static str, FaceUv> {
    cube.faces
        .iter()
        .map(|face| {
            let name = face_name(&face.internal_direction);
            let [u1, v1, u2, v2] = box_rect(name, cube.uv, cube.size, cube.mirror);
            let uv = match name {
                "up" | "down" => FaceUv {
                    uv: [u2, v2],
                    uv_size: [u1 - u2, v1 - v2],
                },
                _ => FaceUv {
                    uv: [u1, v1],
                    uv_size: [u2 - u1, v2 - v1],
                },
            };
            (name, uv)
        })
        .collect()
}

struct Frame {
    pivot: [f64; 3],
    scale: f64,
}

fn convert_part(
    part: &Part,
    path: &str,
    parent: Option<&str>,
    frame: &Frame,
    pose: Option<&BTreeMap<String, PartTransform>>,
    bones: &mut Vec<Bone>,
) {
    let (position, rotation, scale) = match pose.and_then(|parts| parts.get(path)) {
        Some(posed) => (posed.position, posed.rotation, posed.scale),
        None => (part.pivot, part.rotation, part.scale),
    };
    let [k, ky, kz] = scale;
    assert!(k == ky && k == kz, "{path} has a scale that is not uniform");
    let pivot: [f64; 3] =
        std::array::from_fn(|axis| frame.pivot[axis] + frame.scale * position[axis]);
    let own_scale = frame.scale * k;
    let cubes = part
        .cubes
        .iter()
        .map(|cube| {
            let [g, gy, gz] = cube.inflate;
            assert!(
                g == gy && g == gz,
                "{path} has an inflate that is not uniform"
            );
            assert_eq!(cube.uv_scale, [1.0, 1.0], "{path} has a uv_scale");
            assert_eq!(
                cube.faces.len(),
                cube.visible_faces.len(),
                "{path} lists faces that are not visible"
            );
            let corner: [f64; 3] =
                std::array::from_fn(|axis| pivot[axis] + own_scale * cube.corner[axis]);
            let size = cube.size.map(|extent| own_scale * extent);
            let inflate = own_scale * g;
            let boxed = own_scale == 1.0
                && cube.faces.len() == 6
                && cube.size.iter().all(|extent| extent.fract() == 0.0);
            GeometryCube {
                origin: [
                    corner[0],
                    SHAPE_ORIGIN_HEIGHT - (corner[1] + size[1]),
                    corner[2],
                ]
                .map(model_units),
                size: size.map(model_units),
                inflate: (inflate != 0.0).then(|| model_units(inflate)),
                mirror: boxed && cube.mirror,
                uv: if boxed {
                    CubeUv::Box(cube.uv)
                } else {
                    CubeUv::ByFace(per_face_uv(cube))
                },
            }
        })
        .collect();
    bones.push(Bone {
        name: part.name.clone(),
        parent: parent.map(str::to_owned),
        pivot: [pivot[0], SHAPE_ORIGIN_HEIGHT - pivot[1], pivot[2]].map(model_units),
        rotation: rotation.map(|radians| model_units(radians.to_degrees())),
        cubes,
    });
    let frame = Frame {
        pivot,
        scale: own_scale,
    };
    for child in &part.children {
        let child_path = format!("{path}/{}", child.name);
        convert_part(child, &child_path, Some(&part.name), &frame, pose, bones);
    }
}

fn convert(
    entry: &LayerRecord,
    identifier: String,
    pose: Option<&BTreeMap<String, PartTransform>>,
) -> GeometryFile {
    let root = &entry.root;
    let root_pose = pose.and_then(|parts| parts.get("root"));
    let identity = |pivot: [f64; 3], rotation: [f64; 3], scale: [f64; 3]| {
        pivot == [0.0; 3] && rotation == [0.0; 3] && scale == [1.0; 3]
    };
    assert!(
        identity(root.pivot, root.rotation, root.scale) && root.cubes.is_empty(),
        "the top part of {} is not empty",
        entry.model
    );
    assert!(
        root_pose.is_none_or(|posed| identity(posed.position, posed.rotation, posed.scale)),
        "the pose moves the top part of {}",
        entry.model
    );
    let mut bones = Vec::new();
    let frame = Frame {
        pivot: [0.0; 3],
        scale: 1.0,
    };
    for child in &root.children {
        let path = format!("{}/{}", root.name, child.name);
        convert_part(child, &path, None, &frame, pose, &mut bones);
    }
    GeometryFile {
        format_version: "1.12.0",
        geometries: vec![Geometry {
            description: Description {
                identifier,
                texture_width: entry.texture_width,
                texture_height: entry.texture_height,
            },
            bones,
        }],
    }
}

fn geometry_file(artefacts: &Artefacts, source: &GeometrySource) -> String {
    let entry = artefacts.layer_entry(source.model, source.layer);
    let pose = source
        .pose
        .map(|case| artefacts.pose(case, source.model, source.layer));
    to_json(&convert(entry, source.identifier(), pose))
}

fn data_file(block: &BlockShapes) -> String {
    let mut layers = Vec::new();
    for case in &block.cases {
        let when = (!case.conditions.is_empty()).then(|| {
            case.conditions
                .iter()
                .map(|(property, value)| {
                    format!("q.block_state('{property}') == {}", value.molang())
                })
                .collect::<Vec<_>>()
                .join(" && ")
        });
        for shape in &case.shapes {
            let rows = shape.placement.transpose().to_cols_array_2d();
            layers.push(DataLayer {
                when: when.clone(),
                geometry: shape.geometry.identifier(),
                texture: shape.texture.clone(),
                tint: shape.tint,
                surface: shape.surface,
                back_faces: shape.back_faces,
                placement: [rows[0], rows[1], rows[2]]
                    .map(|row| row.map(|value| round(value, 1e-9))),
            });
        }
    }
    to_json(&DataFile { layers })
}

fn to_json<T: Serialize>(value: &T) -> String {
    let pretty = serde_json::to_string_pretty(value).unwrap();
    let mut text = collapse_number_arrays(&pretty);
    text.push('\n');
    text
}

/// Puts each array that holds only numbers and arrays of numbers on one line.
fn collapse_number_arrays(pretty: &str) -> String {
    let bytes = pretty.as_bytes();
    let mut out = String::with_capacity(pretty.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'[' {
            let mut depth = 0;
            let mut end = at;
            for (offset, &byte) in bytes[at..].iter().enumerate() {
                match byte {
                    b'[' => depth += 1,
                    b']' => {
                        depth -= 1;
                        if depth == 0 {
                            end = at + offset;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let inner = &pretty[at..=end];
            if !inner.contains(['{', '"']) {
                let compact: String = inner.split_whitespace().collect();
                out.push_str(&compact.replace(',', ", "));
                at = end + 1;
                continue;
            }
        }
        let next = pretty[at..].chars().next().unwrap();
        out.push(next);
        at += next.len_utf8();
    }
    out
}

/// Each generated file, keyed by its path below `assets/`.
fn generate() -> BTreeMap<PathBuf, String> {
    let artefacts = Artefacts::read();
    let mut files = BTreeMap::new();
    for source in &block_entity_shapes::GEOMETRIES {
        let path = Path::new(GEOMETRY_DIRECTORY).join(format!("{}.json", source.identifier));
        files.insert(path, geometry_file(&artefacts, source));
    }
    for block in block_entity_shapes::all() {
        let path = Path::new(DATA_DIRECTORY).join(format!("{}.json", block.path()));
        files.insert(path, data_file(&block));
    }
    files
}

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn checked_in() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for directory in [GEOMETRY_DIRECTORY, DATA_DIRECTORY] {
        let Ok(entries) = std::fs::read_dir(assets().join(directory)) else {
            continue;
        };
        for entry in entries {
            let name = entry.unwrap().file_name();
            found.push(Path::new(directory).join(name));
        }
    }
    found
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a developer asks for the files to be written again through the environment"
)]
fn the_block_entity_files_are_generated_from_the_artefacts() {
    let generated = generate();
    let stale: Vec<PathBuf> = checked_in()
        .into_iter()
        .filter(|path| !generated.contains_key(path))
        .collect();
    if std::env::var("MCRS_BLESS").is_ok() {
        for path in &stale {
            std::fs::remove_file(assets().join(path)).unwrap();
        }
        for (path, text) in &generated {
            let target = assets().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, text).unwrap();
        }
        return;
    }
    assert!(stale.is_empty(), "files that no shape writes: {stale:?}");
    for (path, text) in &generated {
        let held = std::fs::read_to_string(assets().join(path)).unwrap_or_default();
        assert!(
            held == *text,
            "assets/{} is stale; rerun with MCRS_BLESS=1 to write it again",
            path.display()
        );
    }
}
