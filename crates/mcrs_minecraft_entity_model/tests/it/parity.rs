//! Bakes each layer entry of the model artefact and compares the faces with the faces that the
//! reference computes.

use std::collections::BTreeMap;

use glam::{DVec2, DVec3, Vec2, Vec3};
use mcrs_minecraft_entity_model::{
    BakedFace, BakedGeometry, CUBOID_EXPLICIT, CUBOID_MIRROR, CuboidRecords, FACE_TRIANGLES, Face,
    GeometryFile, INFLATE_STEP,
};

use crate::artefact::{self, LayerRecord, ModelArtefact, Part};
use crate::convert::{self, Frame, ORIGIN_HEIGHT, Rules};

const TOLERANCE: f64 = 1e-4;
/// The 32-byte record rounds the inflate to a step of 1/8192 on each axis, which moves a
/// corner by up to 1.06e-4. The bound of the block-entity parity rule is 1e-5 block.
const RECORD_TOLERANCE: f64 = 1.6e-4;

#[derive(Default)]
struct Errors {
    position: f64,
    texel: f64,
    faces: usize,
}

#[derive(Default)]
struct Explicit {
    cuboids: usize,
    faces: usize,
}

/// The internal point of a vertex of the part with `frame`.
fn internal_vertex(frame: Frame, vertex: [f64; 3]) -> DVec3 {
    let [x, y, z] = frame.place(vertex);
    DVec3::new(-x, ORIGIN_HEIGHT - y, z)
}

fn bake(entry: &LayerRecord, rules: Rules) -> Result<BakedGeometry, String> {
    let geometry = convert::convert(entry, "geometry.parity", rules)?;
    let text = convert::to_json(&convert::GeometryFile {
        format_version: "1.12.0",
        geometries: vec![geometry],
    });
    let file: GeometryFile = text.parse().unwrap();
    let mut diagnostics = file.diagnostics;
    let baked = file.geometries[0].bake(&mut diagnostics);
    baked.map_err(|error| error.to_string())
}

fn compare_layer(entry: &LayerRecord, baked: &BakedGeometry, errors: &mut Errors) {
    let mut cubes_of_bone: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, cube) in baked.cubes.iter().enumerate() {
        cubes_of_bone.entry(cube.bone).or_default().push(index);
    }
    let root = Frame {
        pivot: [0.0; 3],
        scale: entry.root.scale[0],
    };
    let mut parts = Vec::new();
    for child in &entry.root.children {
        collect_parts(child, root, &mut parts);
    }
    let label = format!("{} / {}", entry.model, entry.layer);
    for (part, frame) in parts {
        let bone = baked.bone_index(&part.name).unwrap();
        let cubes = cubes_of_bone.get(&bone).cloned().unwrap_or_default();
        assert_eq!(
            cubes.len(),
            part.cubes.len(),
            "{label}: cubes of {}",
            part.name
        );
        for (cube, &index) in part.cubes.iter().zip(&cubes) {
            let ours = &baked.cubes[index];
            assert_eq!(
                ours.faces.len(),
                cube.faces.len(),
                "{label}: faces of {}",
                part.name
            );
            for face in &cube.faces {
                let name: Face = face.internal_direction.parse().unwrap();
                let baked_face = ours
                    .faces
                    .iter()
                    .find(|candidate| candidate.face == name)
                    .unwrap();
                compare_face(&label, frame, face, baked_face, baked, errors);
            }
        }
    }
}

fn collect_parts<'a>(part: &'a Part, parent: Frame, parts: &mut Vec<(&'a Part, Frame)>) {
    let frame = parent.child(part);
    parts.push((part, frame));
    for child in &part.children {
        collect_parts(child, frame, parts);
    }
}

fn compare_face(
    label: &str,
    frame: Frame,
    face: &artefact::Face,
    ours: &BakedFace,
    baked: &BakedGeometry,
    errors: &mut Errors,
) {
    errors.faces += 1;
    let [nx, ny, nz] = face.normal;
    let normal = DVec3::new(-nx, -ny, nz);
    assert!(
        ours.normal.as_dvec3().distance(normal) < TOLERANCE,
        "{label}: the normal of {} is {} and not {normal}",
        face.internal_direction,
        ours.normal
    );
    let texture = baked.texture_size.as_dvec2();
    let mut loop_points = Vec::new();
    for vertex in &face.vertices {
        let [Some(x), Some(y), Some(z), u, v] = *vertex else {
            panic!("{label}: a vertex has no position");
        };
        let expected = internal_vertex(frame, [x, y, z]);
        loop_points.push(expected);
        let distances = ours
            .corners
            .map(|corner| corner.as_dvec3().distance(expected));
        let nearest = distances.iter().copied().fold(f64::INFINITY, f64::min);
        errors.position = errors.position.max(nearest);
        let (Some(u), Some(v)) = (u, v) else {
            continue;
        };
        let texel = (0..4)
            .filter(|&corner| distances[corner] <= nearest + TOLERANCE)
            .map(|corner| (ours.uvs[corner].as_dvec2() * texture).distance(DVec2::new(u, v)))
            .fold(f64::INFINITY, f64::min);
        errors.texel = errors.texel.max(texel);
    }
    let reference = (loop_points[1] - loop_points[0]).cross(loop_points[2] - loop_points[0]);
    let [c0, c1, c2] = FACE_TRIANGLES[0].map(|corner| ours.corners[corner].as_dvec3());
    let winding = (c1 - c0).cross(c2 - c0);
    if reference.length() > TOLERANCE && winding.length() > TOLERANCE {
        assert!(
            winding.dot(reference) > 0.0 && winding.dot(normal) > 0.0,
            "{label}: the {} face turns the wrong way",
            face.internal_direction
        );
    }
}

/// The faces that the GPU rebuilds from the cuboid records of `baked`, in place of the faces of
/// the bake. A face with no area keeps the normal of the bake: the records give it none.
fn rebuild_from_records(baked: &BakedGeometry, explicit: &mut Explicit) -> BakedGeometry {
    let records = CuboidRecords::new(baked).unwrap();
    assert_eq!(
        records.cuboids.len(),
        baked.cubes.len(),
        "one record per cube"
    );
    let mut rebuilt = baked.clone();
    let texture = baked.texture_size;
    for (cube, record) in rebuilt.cubes.iter_mut().zip(&records.cuboids) {
        let mask = record.flags & 0x3f;
        let drawn: Vec<Face> = Face::ALL
            .into_iter()
            .filter(|face| mask & face.mask_bit() != 0)
            .collect();
        assert_eq!(
            drawn,
            cube.faces.iter().map(|face| face.face).collect::<Vec<_>>(),
            "the face mask names the drawn faces"
        );
        if let Some(first) = record.first_explicit_face() {
            explicit.cuboids += 1;
            assert_eq!(record.flags & CUBOID_EXPLICIT, CUBOID_EXPLICIT);
            for (offset, face) in cube.faces.iter_mut().enumerate() {
                let stored = records.explicit_faces[first as usize + offset];
                assert_eq!(usize::from(stored.bone), cube.bone, "the bone of a face");
                explicit.faces += 1;
                let corner = Vec3::from(stored.corner);
                let [edge_1, edge_2] = [stored.edge_1, stored.edge_2].map(Vec3::from);
                face.corners = [
                    corner,
                    corner + edge_2,
                    corner + edge_1,
                    corner + edge_1 + edge_2,
                ];
                let uv = Vec2::from(stored.uv);
                let [uv_1, uv_2] = [stored.uv_edge_1, stored.uv_edge_2].map(Vec2::from);
                face.uvs = [uv, uv + uv_2, uv + uv_1, uv + uv_1 + uv_2];
                let cross = edge_1.cross(edge_2);
                if cross.length() > 1e-6 {
                    face.normal = cross.normalize();
                }
            }
            continue;
        }
        assert_eq!(usize::from(record.bone), cube.bone, "the bone of a cuboid");
        let origin = Vec3::from(record.origin);
        let size = Vec3::from(record.size);
        let inflate = f32::from(record.inflate) * INFLATE_STEP;
        let mut lo = origin - inflate;
        let mut hi = origin + size + inflate;
        let flat = lo.cmpgt(hi);
        lo = Vec3::select(flat, origin + size / 2.0, lo);
        hi = Vec3::select(flat, origin + size / 2.0, hi);
        let mirror = record.flags & CUBOID_MIRROR != 0;
        let uv = [f64::from(record.uv[0]), f64::from(record.uv[1])];
        let layout = size.as_dvec3().floor().to_array();
        for face in &mut cube.faces {
            face.corners = face.face.corners(lo, hi);
            face.normal = face.face.normal();
            let [u1, v1, u2, v2] =
                convert::box_rect(face.face.name(), uv, layout, mirror).map(|texel| texel as f32);
            face.uvs = [
                Vec2::new(u1, v1),
                Vec2::new(u2, v1),
                Vec2::new(u1, v2),
                Vec2::new(u2, v2),
            ]
            .map(|texel| texel / texture);
        }
    }
    rebuilt
}

fn check(label: &str, errors: &Errors, position_tolerance: f64) {
    assert!(
        errors.position <= position_tolerance && errors.texel <= TOLERANCE,
        "{label}: position error {}, texel error {}",
        errors.position,
        errors.texel
    );
}

#[test]
fn the_pig_and_the_frog_bake_to_the_faces_of_the_artefact() {
    let artefact = ModelArtefact::read();
    for (model, layer, faces) in [
        ("minecraft:pig", "main", 42),
        ("minecraft:pig", "saddle", 42),
        ("minecraft:cold_pig", "main", 48),
        ("minecraft:pig_baby", "main", 42),
        ("minecraft:frog", "main", 96),
    ] {
        let entry = artefact.layer_entry(model, layer);
        let baked = bake(entry, Rules::Mob).unwrap();
        let mut explicit = Explicit::default();
        let rebuilt = rebuild_from_records(&baked, &mut explicit);
        assert_eq!(
            explicit.cuboids, 0,
            "{model} / {layer} uses only the 32-byte record"
        );
        let mut errors = Errors::default();
        compare_layer(entry, &rebuilt, &mut errors);
        check(
            &format!("{model} / {layer}: records"),
            &errors,
            RECORD_TOLERANCE,
        );
        let mut errors = Errors::default();
        compare_layer(entry, &baked, &mut errors);
        assert_eq!(errors.faces, faces, "{model} / {layer}: face count");
        assert_eq!(
            baked.faces().count(),
            faces,
            "{model} / {layer}: baked face count"
        );
        check(&format!("{model} / {layer}"), &errors, TOLERANCE);
    }
}

#[test]
fn each_layer_entry_that_converts_bakes_and_rebuilds_from_its_records_to_the_artefact() {
    let artefact = ModelArtefact::read();
    let mut compared = 0;
    let mut refused = Vec::new();
    let mut baked_errors = Errors::default();
    let mut record_errors = Errors::default();
    let mut explicit = Explicit::default();
    for entry in &artefact.layers {
        let label = format!("{} / {}", entry.model, entry.layer);
        let baked = match bake(entry, Rules::BlockEntity) {
            Ok(baked) => baked,
            Err(refusal) => {
                refused.push(format!("{label}: {refusal}"));
                continue;
            }
        };
        let rebuilt = rebuild_from_records(&baked, &mut explicit);
        for (model, total, tolerance) in [
            (&baked, &mut baked_errors, TOLERANCE),
            (&rebuilt, &mut record_errors, RECORD_TOLERANCE),
        ] {
            let mut errors = Errors::default();
            compare_layer(entry, model, &mut errors);
            check(&label, &errors, tolerance);
            total.position = total.position.max(errors.position);
            total.texel = total.texel.max(errors.texel);
            total.faces += errors.faces;
        }
        compared += 1;
    }
    println!(
        "compared {compared} of {} layer entries, {} faces; bake: max position error {:e}, \
         max texel error {:e}; records: max position error {:e}, max texel error {:e}; \
         {} explicit cuboids with {} faces; refused {}:\n{}",
        artefact.layers.len(),
        baked_errors.faces,
        baked_errors.position,
        baked_errors.texel,
        record_errors.position,
        record_errors.texel,
        explicit.cuboids,
        explicit.faces,
        refused.len(),
        refused.join("\n")
    );
    assert!(explicit.cuboids > 0, "some cubes need explicit faces");
    assert!(
        refused.iter().all(|refusal| {
            ["not uniform", "uv_scale", "top part"]
                .iter()
                .any(|condition| refusal.contains(condition))
        }),
        "only a scale or an inflate that is not uniform, a uv_scale or a top part that is not \
         empty refuses a layer: {refused:?}"
    );
}
