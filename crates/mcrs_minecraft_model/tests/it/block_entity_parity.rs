//! The baked faces of each static block entity agree with the faces and the placements that
//! the reference reports in the oracle artefacts.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use bevy::math::{DMat3, DMat4, DVec3, Vec3};
use mcrs_minecraft_model::block_entity_model::{
    BlockEntityModels, DATA_DIRECTORY, GEOMETRY_DIRECTORY, ShapeQuad,
};

use crate::artefact::{Artefacts, Part, PartTransform};
use crate::block_entity_shapes::{self, BlockShapes, Literal, Shape, block_properties};

const VERTEX_TOLERANCE: f64 = 1e-5;
const PLACEMENT_TOLERANCE: f64 = 1e-6;

struct ExpectedFace {
    texture: String,
    dye: Option<u8>,
    /// Each corner in block space with its texture coordinate in sprite space.
    corners: Vec<(DVec3, [f64; 2])>,
    normal: DVec3,
    back_faces: bool,
}

fn read_tree(root: &Path, directory: &str, files: &mut HashMap<String, Vec<u8>>) {
    let mut pending = vec![root.join(directory)];
    while let Some(at) = pending.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let key = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            files.insert(key, std::fs::read(&path).unwrap());
        }
    }
}

fn models(blocks: &[BlockShapes]) -> BlockEntityModels {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut files = HashMap::new();
    read_tree(&root, DATA_DIRECTORY, &mut files);
    read_tree(&root, GEOMETRY_DIRECTORY, &mut files);
    for block in blocks {
        let key = format!("mcrs/block_definition/{}.json", block.path());
        files.insert(key.clone(), std::fs::read(root.join(&key)).unwrap());
    }
    BlockEntityModels::from_files(&files)
}

fn part_transform(position: [f64; 3], rotation: [f64; 3], scale: [f64; 3]) -> DMat4 {
    DMat4::from_translation(DVec3::from(position))
        * DMat4::from_rotation_z(rotation[2])
        * DMat4::from_rotation_y(rotation[1])
        * DMat4::from_rotation_x(rotation[0])
        * DMat4::from_scale(DVec3::from(scale))
}

struct Walk<'a> {
    shape: &'a Shape,
    pose: Option<&'a BTreeMap<String, PartTransform>>,
    placement: DMat4,
    texture_side: f64,
}

impl Walk<'_> {
    fn faces(&self, part: &Part, path: &str, parent: DMat4, out: &mut Vec<ExpectedFace>) {
        let local = match self.pose.and_then(|parts| parts.get(path)) {
            Some(posed) => part_transform(posed.position, posed.rotation, posed.scale),
            None => part_transform(part.pivot, part.rotation, part.scale),
        };
        let model = parent * local;
        let to_block = self.placement * DMat4::from_scale(DVec3::splat(1.0 / 16.0)) * model;
        let normals = DMat3::from_mat4(to_block).inverse().transpose();
        for cube in &part.cubes {
            for face in &cube.faces {
                let corners = face
                    .vertices
                    .iter()
                    .map(|vertex| {
                        let [x, y, z, u, v] = vertex.map(|value| value.unwrap());
                        let at = to_block.transform_point3(DVec3::new(x, y, z));
                        (at, [u / self.texture_side, v / self.texture_side])
                    })
                    .collect();
                out.push(ExpectedFace {
                    texture: self.shape.texture.clone(),
                    dye: self.shape.tint,
                    corners,
                    normal: (normals * DVec3::from(face.normal)).normalize(),
                    back_faces: self.shape.back_faces,
                });
            }
        }
        for child in &part.children {
            self.faces(child, &format!("{path}/{}", child.name), model, out);
        }
    }
}

fn expected_faces(artefacts: &Artefacts, shape: &Shape) -> Vec<ExpectedFace> {
    let source = shape.geometry;
    let entry = artefacts.layer_entry(source.model, source.layer);
    let placement = match &shape.artefact_placement {
        Some(reference) => {
            let rows = artefacts.placement(reference.shape, &reference.state);
            let matrix = DMat4::from_cols_array_2d(&rows).transpose();
            let formula = shape.placement.to_cols_array();
            for (ours, theirs) in formula.iter().zip(matrix.to_cols_array()) {
                assert!(
                    (ours - theirs).abs() <= PLACEMENT_TOLERANCE,
                    "the placement of {} {} is {:?}, the reference has {:?}",
                    reference.shape,
                    reference.state,
                    shape.placement,
                    matrix
                );
            }
            matrix
        }
        None => shape.placement,
    };
    let walk = Walk {
        shape,
        pose: source
            .pose
            .map(|case| artefacts.pose(case, source.model, source.layer)),
        placement,
        texture_side: entry.texture_width.max(entry.texture_height),
    };
    let mut faces = Vec::new();
    walk.faces(&entry.root, &entry.root.name, DMat4::IDENTITY, &mut faces);
    faces
}

fn states(path: &str) -> Vec<Vec<(String, String)>> {
    let mut states = vec![Vec::new()];
    for (name, values) in block_properties(path) {
        let mut next = Vec::with_capacity(states.len() * values.len());
        for state in &states {
            for value in &values {
                let mut extended: Vec<(String, String)> = state.clone();
                extended.push((name.clone(), value.clone()));
                next.push(extended);
            }
        }
        states = next;
    }
    states
}

fn holds(conditions: &[(&str, Literal)], state: &[(String, String)]) -> bool {
    conditions.iter().all(|(property, literal)| {
        state
            .iter()
            .any(|(name, value)| name == property && *value == literal.text())
    })
}

fn close(baked: Vec3, expected: DVec3) -> bool {
    (baked.as_dvec3() - expected).abs().max_element() <= VERTEX_TOLERANCE
}

fn matches(quad: &ShapeQuad<'_>, face: &ExpectedFace) -> Option<f64> {
    if quad.texture != face.texture || quad.dye != face.dye {
        return None;
    }
    let all_found = face.corners.iter().all(|(at, uv)| {
        quad.positions
            .iter()
            .zip(&quad.uvs)
            .any(|(position, coordinate)| {
                close(*position, *at)
                    && (f64::from(coordinate[0]) - uv[0]).abs() <= VERTEX_TOLERANCE
                    && (f64::from(coordinate[1]) - uv[1]).abs() <= VERTEX_TOLERANCE
            })
    });
    let [p0, p1, p2, p3] = quad.positions.map(|position| position.as_dvec3());
    all_found.then(|| (p2 - p0).cross(p3 - p1).dot(face.normal))
}

/// Takes from `unused` the quads that draw `face`: one quad that faces the artefact normal,
/// and a second one that faces away from it when the face shows from both sides.
fn take_quads(
    quads: &[ShapeQuad<'_>],
    unused: &mut Vec<usize>,
    face: &ExpectedFace,
) -> Result<(), String> {
    let [a, b, c, d] = [0, 1, 2, 3].map(|index| face.corners[index].0);
    let flat = (c - a).cross(d - b).length() <= 1e-9;
    let sides: &[f64] = if face.back_faces {
        &[1.0, -1.0]
    } else {
        &[1.0]
    };
    for &side in sides {
        let found = unused.iter().position(|&quad| {
            quads
                .get(quad)
                .and_then(|quad| matches(quad, face))
                .is_some_and(|facing| flat || facing * side > 0.0)
        });
        match found {
            Some(index) => {
                unused.remove(index);
            }
            None => {
                return Err(format!(
                    "no quad draws the face {:?} on the side {side}",
                    face.corners
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn baked_static_block_entities_agree_with_the_artefacts() {
    let artefacts = Artefacts::read();
    let blocks = block_entity_shapes::all();
    let models = models(&blocks);
    assert_eq!(models.errors(), [] as [String; 0]);
    let mut failures = Vec::new();
    for block in &blocks {
        for state in states(block.path()) {
            let props: Vec<(&str, &str)> = state
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            let expected: Vec<ExpectedFace> = block
                .cases
                .iter()
                .filter(|case| holds(&case.conditions, &state))
                .flat_map(|case| &case.shapes)
                .flat_map(|shape| expected_faces(&artefacts, shape))
                .collect();
            let quads = models.bake(&block.block, &props);
            let mut unused: Vec<usize> = (0..quads.len()).collect();
            for face in &expected {
                if let Err(error) = take_quads(&quads, &mut unused, face) {
                    failures.push(format!("{} {state:?}: {error}", block.block));
                    break;
                }
            }
            if !unused.is_empty() {
                failures.push(format!(
                    "{} {state:?} draws {} faces that the artefact does not have",
                    block.block,
                    unused.len()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
