//! The baked faces of each model file and variant turn agree with the face artefact of the
//! reference client.

use bevy::math::DVec3;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_model::face_bake::{ElementFace, VariantTransform, bake_elements};
use mcrs_minecraft_model::model_file::{ModelFile, direction_named};
use mcrs_minecraft_model::resource_set::resource_path;
use mcrs_minecraft_fixtures::fixture;
use serde::Deserialize;

use crate::client_pack::client_pack;

const TOLERANCE: f64 = 1e-5;

#[derive(Deserialize)]
struct FacesArtefact {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    model: String,
    x: u16,
    y: u16,
    z: u16,
    uvlock: bool,
    faces: Vec<ExpectedFace>,
}

#[derive(Deserialize)]
struct ExpectedFace {
    texture: String,
    direction: String,
    cull: Option<String>,
    tint: i64,
    shade_face: Option<String>,
    light_emission: u8,
    positions: Vec<[f64; 3]>,
    uv: Vec<[f64; 2]>,
}

fn named(name: Option<&str>) -> Option<Direction> {
    name.map(|name| direction_named(name).unwrap())
}

/// Why `ours` disagrees with `expected`, or `None` when it agrees.
fn disagreement(ours: &ElementFace, expected: &ExpectedFace) -> Option<String> {
    if ours.texture != expected.texture
        || ours.tint.map_or(-1, i64::from) != expected.tint
        || ours.shade_face != named(expected.shade_face.as_deref())
        || ours.light_emission != expected.light_emission
    {
        return Some(format!("texture, tint, shade face or emission: {ours:?}"));
    }
    let corners_agree = ours
        .positions
        .iter()
        .zip(&expected.positions)
        .all(|(a, b)| (a.as_dvec3() - DVec3::from_array(*b)).abs().max_element() <= TOLERANCE);
    let uvs_agree = ours.uvs.iter().zip(&expected.uv).all(|(a, b)| {
        (f64::from(a[0]) - b[0]).abs() <= TOLERANCE && (f64::from(a[1]) - b[1]).abs() <= TOLERANCE
    });
    if !corners_agree || !uvs_agree {
        return Some(format!("corners or uv: {ours:?}"));
    }
    let theirs = named(Some(&expected.direction))?;
    if ours.direction != theirs && !tied(ours, theirs) {
        return Some(format!(
            "direction {:?}, expected {theirs:?}",
            ours.direction
        ));
    }
    None
}

/// Whether the face sits at the same angle to both directions, which the reference resolves by
/// its rounding.
fn tied(face: &ElementFace, other: Direction) -> bool {
    let [p0, p1, p2, _] = face.positions.map(|p| p.as_dvec3());
    let normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
    let dot = |direction: Direction| normal.dot(direction.normal().as_dvec3());
    (dot(face.direction) - dot(other)).abs() <= TOLERANCE
}

#[test]
fn baked_faces_agree_with_the_reference_client() {
    let artefact: FacesArtefact = serde_json::from_str(&fixture("block_faces").text()).unwrap();
    let mut failures = Vec::new();
    for case in &artefact.cases {
        let label = format!(
            "{} x={} y={} z={} uvlock={}",
            case.model, case.x, case.y, case.z, case.uvlock
        );
        let path = resource_path(&case.model, "models", "json");
        let file = ModelFile::parse(client_pack().read(&path).unwrap()).unwrap();
        let elements = file.elements.unwrap_or_default();
        let transform = VariantTransform {
            x: case.x,
            y: case.y,
            z: case.z,
            uvlock: case.uvlock,
        };
        let ours = match bake_elements(&elements, transform) {
            Ok(faces) => faces,
            Err(error) => {
                failures.push(format!("{label}: {error}"));
                continue;
            }
        };
        let culls: Vec<Option<Direction>> = std::iter::once(None)
            .chain(Direction::all().map(Some))
            .collect();
        for cull in culls {
            let ours: Vec<&ElementFace> = ours.iter().filter(|face| face.cull == cull).collect();
            let theirs: Vec<&ExpectedFace> = case
                .faces
                .iter()
                .filter(|face| named(face.cull.as_deref()) == cull)
                .collect();
            if ours.len() != theirs.len() {
                failures.push(format!(
                    "{label}: {} faces with cull {cull:?}, expected {}",
                    ours.len(),
                    theirs.len()
                ));
                continue;
            }
            for (index, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
                if let Some(reason) = disagreement(ours, theirs) {
                    failures.push(format!(
                        "{label}: face {index} with cull {cull:?}: {reason}"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} faces disagree, first {:#?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}
