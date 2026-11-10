//! The conversion of a layer entry of the model artefact to a geometry of the current shape.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::artefact::{Cube, LayerRecord, Part};

/// The file `y` of the artefact origin.
pub const ORIGIN_HEIGHT: f64 = 24.0;

/// Which layer entries the conversion accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rules {
    /// The conversion of a mob layer entry: box UV only.
    Mob,
    /// The mob conversion, a uniform part scale, and the per-face UV for a scaled cube, a cube
    /// with fewer than six visible faces or with a size that is not whole.
    BlockEntity,
}

#[derive(Serialize)]
pub struct GeometryFile {
    pub format_version: &'static str,
    #[serde(rename = "minecraft:geometry")]
    pub geometries: Vec<Geometry>,
}

#[derive(Serialize)]
pub struct Geometry {
    pub description: Description,
    pub bones: Vec<Bone>,
}

#[derive(Serialize)]
pub struct Description {
    pub identifier: String,
    pub texture_width: f64,
    pub texture_height: f64,
}

#[derive(Serialize)]
pub struct Bone {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub pivot: [f64; 3],
    #[serde(skip_serializing_if = "is_zero")]
    pub rotation: [f64; 3],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cubes: Vec<GeometryCube>,
}

#[derive(Serialize)]
pub struct GeometryCube {
    pub origin: [f64; 3],
    pub size: [f64; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inflate: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mirror: bool,
    pub uv: CubeUv,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum CubeUv {
    Box([f64; 2]),
    ByFace(BTreeMap<String, FaceUv>),
}

#[derive(Serialize)]
pub struct FaceUv {
    pub uv: [f64; 2],
    pub uv_size: [f64; 2],
}

fn is_zero(values: &[f64; 3]) -> bool {
    values.iter().all(|value| *value == 0.0)
}

fn model_units(value: f64) -> f64 {
    (value * 1e6).round() / 1e6 + 0.0
}

/// Converts `entry`, or names the condition that the conversion refuses.
pub fn convert(entry: &LayerRecord, identifier: &str, rules: Rules) -> Result<Geometry, String> {
    let root = &entry.root;
    if root.pivot != [0.0; 3] || root.rotation != [0.0; 3] || !root.cubes.is_empty() {
        return Err("the top part is not empty".to_owned());
    }
    let scale = part_scale(root, rules)?;
    let mut bones = Vec::new();
    for child in &root.children {
        let frame = Frame {
            pivot: [0.0; 3],
            scale,
        };
        convert_part(child, None, frame, rules, &mut bones)?;
    }
    Ok(Geometry {
        description: Description {
            identifier: identifier.to_owned(),
            texture_width: entry.texture_width,
            texture_height: entry.texture_height,
        },
        bones,
    })
}

/// The absolute pivot of a part and the product of its scale and the scales of the parts
/// above it.
#[derive(Clone, Copy)]
pub struct Frame {
    pub pivot: [f64; 3],
    pub scale: f64,
}

impl Frame {
    /// The frame of `part`, a child of the part of this frame.
    pub fn child(self, part: &Part) -> Self {
        Self {
            pivot: std::array::from_fn(|axis| self.pivot[axis] + self.scale * part.pivot[axis]),
            scale: self.scale * part.scale[0],
        }
    }

    /// The absolute point of `point` in the frame of the part.
    pub fn place(self, point: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| self.pivot[axis] + self.scale * point[axis])
    }
}

fn part_scale(part: &Part, rules: Rules) -> Result<f64, String> {
    let [k, k_y, k_z] = part.scale;
    if k == 1.0 && k_y == 1.0 && k_z == 1.0 {
        return Ok(1.0);
    }
    if rules == Rules::Mob {
        return Err(format!("the part {} has a scale", part.name));
    }
    if k != k_y || k != k_z {
        return Err(format!(
            "the part {} has a scale that is not uniform",
            part.name
        ));
    }
    Ok(k)
}

fn convert_part(
    part: &Part,
    parent: Option<&str>,
    parent_frame: Frame,
    rules: Rules,
    bones: &mut Vec<Bone>,
) -> Result<(), String> {
    part_scale(part, rules)?;
    let frame = parent_frame.child(part);
    let pivot = frame.pivot;
    let cubes = part
        .cubes
        .iter()
        .map(|cube| convert_cube(cube, frame, rules))
        .collect::<Result<Vec<_>, _>>()?;
    bones.push(Bone {
        name: part.name.clone(),
        parent: parent.map(str::to_owned),
        pivot: [pivot[0], ORIGIN_HEIGHT - pivot[1], pivot[2]].map(model_units),
        rotation: part
            .rotation
            .map(|radians| model_units(radians.to_degrees())),
        cubes,
    });
    for child in &part.children {
        convert_part(child, Some(&part.name), frame, rules, bones)?;
    }
    Ok(())
}

fn convert_cube(cube: &Cube, frame: Frame, rules: Rules) -> Result<GeometryCube, String> {
    let [inflate, inflate_y, inflate_z] = cube.inflate;
    if inflate != inflate_y || inflate != inflate_z {
        return Err("an inflate is not uniform".to_owned());
    }
    if cube.uv_scale != [1.0, 1.0] {
        return Err("a cube has a uv_scale".to_owned());
    }
    let whole = frame.scale == 1.0
        && cube.visible_faces.len() == 6
        && cube.size.iter().all(|extent| extent.fract() == 0.0);
    if !whole && rules == Rules::Mob {
        return Err("a cube needs the per-face UV".to_owned());
    }
    let corner = frame.place(cube.corner);
    let size = cube.size.map(|extent| frame.scale * extent);
    let inflate = frame.scale * inflate;
    Ok(GeometryCube {
        origin: [corner[0], ORIGIN_HEIGHT - (corner[1] + size[1]), corner[2]].map(model_units),
        size: size.map(model_units),
        inflate: (inflate != 0.0).then(|| model_units(inflate)),
        mirror: whole && cube.mirror,
        uv: if whole {
            CubeUv::Box(cube.uv)
        } else {
            CubeUv::ByFace(per_face_uv(cube))
        },
    })
}

/// The box UV rectangle `[u1, v1, u2, v2]` of a face named in internal space, laid out from the
/// exact size.
pub fn box_rect(face: &str, [u, v]: [f64; 2], [w, h, d]: [f64; 3], mirror: bool) -> [f64; 4] {
    let laid_out = match (mirror, face) {
        (true, "east") => "west",
        (true, "west") => "east",
        (_, face) => face,
    };
    let [u1, v1, u2, v2] = match laid_out {
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

fn per_face_uv(cube: &Cube) -> BTreeMap<String, FaceUv> {
    let mut faces = BTreeMap::new();
    for face in &cube.faces {
        let name = face.internal_direction.as_str();
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
        faces.insert(name.to_owned(), uv);
    }
    faces
}

pub fn to_json<T: Serialize>(value: &T) -> String {
    let pretty = serde_json::to_string_pretty(value).unwrap();
    let mut text = collapse_number_arrays(&pretty);
    text.push('\n');
    text
}

/// Puts each array that holds only numbers on one line.
fn collapse_number_arrays(pretty: &str) -> String {
    let mut out = String::with_capacity(pretty.len());
    let mut rest = pretty;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let close = after.find(']').unwrap();
        let inner = &after[..=close];
        if inner[1..].contains(['[', '{', '"']) {
            out.push('[');
            rest = &after[1..];
            continue;
        }
        let compact: String = inner.split_whitespace().collect();
        out.push_str(&compact.replace(',', ", "));
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}
