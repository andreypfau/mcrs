//! Static block entities: blocks whose shape comes from an entity geometry, baked into the
//! faces of the block as described by the data files under `mcrs/block_entity_model/`.

use std::collections::HashMap;

use bevy::math::{Affine3A, Mat3, Mat4, Vec2, Vec3, Vec4};
use mcrs_minecraft_block::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_block::definition::schema::{BlockDefinitionFile, BlockProperties};
use mcrs_minecraft_block::definition::state_condition::{StateCondition, StateInputs};
use mcrs_minecraft_model_file::property_table::PropertyTable;
use mcrs_molang::ScalarVm;
use serde::Deserialize;

use mcrs_minecraft_core::Direction;
use mcrs_minecraft_entity_model::{
    BakedGeometry, Diagnostic, Geometry, GeometryFile, resolve_parents,
};

use crate::bake_error::BakeError;
use crate::block_entity_error::{BlockEntityError, BlockEntityErrorKind, LayerError};

/// The pack directory of the data files, one file `<namespace>/<path>.json` per block.
pub const DATA_DIRECTORY: &str = "mcrs/block_entity_model";
/// The pack directory of the geometry files of the entity pack.
pub const GEOMETRY_DIRECTORY: &str = "mcrs/entity/geometry";

const DYE_COUNT: u8 = 16;
const MODEL_UNITS_PER_BLOCK: f32 = 16.0;
const SHAPE_ORIGIN_HEIGHT: f32 = 24.0;

/// The layers of each static block entity of a pack, with the geometries they draw.
#[derive(Debug, Default)]
pub struct BlockEntityModels {
    blocks: HashMap<String, StaticBlockEntity>,
    geometries: Vec<RestGeometry>,
    errors: Vec<BlockEntityError>,
}

/// A baked geometry with the model transform of each bone in the rest pose.
#[derive(Debug)]
struct RestGeometry {
    baked: BakedGeometry,
    bone_transforms: Vec<Affine3A>,
}

#[derive(Debug)]
struct StaticBlockEntity {
    properties: BlockProperties,
    table: PropertyTable,
    layers: Vec<Layer>,
}

#[derive(Debug)]
struct Layer {
    when: Option<StateCondition>,
    geometry: usize,
    texture: String,
    dye: Option<u8>,
    surface: Surface,
    back_faces: bool,
    placement: Mat4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataFile {
    layers: Vec<LayerRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerRecord {
    when: Option<String>,
    geometry: String,
    texture: String,
    tint: Option<u8>,
    surface: Surface,
    back_faces: bool,
    placement: [[f32; 4]; 3],
}

/// How the texels of a layer combine with what is behind them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    /// The alpha of the texture is ignored.
    Opaque,
    /// A texel with an alpha below 0.1 is not drawn; the other texels are opaque.
    Cutout,
    /// A texel blends by its alpha; a texel with an alpha below 0.1 is not drawn.
    Blend,
}

impl BlockEntityModels {
    /// Reads the data files, the geometry files and the block definitions they need from
    /// `files`, keyed by their path in the pack. A defect in a file is a content error: the
    /// file is skipped and `errors` names it.
    pub fn from_files(files: &HashMap<String, Vec<u8>>) -> Self {
        let mut models = Self::default();
        let mut paths: HashMap<String, &str> = HashMap::new();
        let read = read_geometries(files, &mut paths, &mut models.errors);
        let path_of = |identifier: &str| {
            paths
                .get(&identifier.to_ascii_lowercase())
                .copied()
                .unwrap_or(GEOMETRY_DIRECTORY)
        };
        let mut rejected = Vec::new();
        let resolved = resolve_parents(read, &mut rejected);
        for diagnostic in &rejected {
            if let Diagnostic::RejectedGeometry {
                identifier: Some(identifier),
                ..
            } = diagnostic
            {
                models.errors.push(BlockEntityError {
                    path: path_of(identifier).to_owned(),
                    kind: BlockEntityErrorKind::Geometry {
                        identifier: None,
                        message: diagnostic.to_string(),
                    },
                });
            }
        }
        let mut by_identifier: HashMap<String, usize> = HashMap::new();
        for geometry in resolved {
            let path = path_of(&geometry.identifier);
            let mut diagnostics = Vec::new();
            match geometry.bake(&mut diagnostics) {
                Ok(baked) => {
                    by_identifier.insert(
                        baked.identifier.to_ascii_lowercase(),
                        models.geometries.len(),
                    );
                    let bone_transforms = baked.model_transforms(&[]);
                    models.geometries.push(RestGeometry {
                        baked,
                        bone_transforms,
                    });
                }
                Err(error) => models.errors.push(BlockEntityError {
                    path: path.to_owned(),
                    kind: BlockEntityErrorKind::Geometry {
                        identifier: Some(geometry.identifier.clone()),
                        message: error.to_string(),
                    },
                }),
            }
            models.errors.extend(
                diagnostics
                    .iter()
                    .filter(|diagnostic| is_content_error(diagnostic))
                    .map(|diagnostic| BlockEntityError {
                        path: path.to_owned(),
                        kind: BlockEntityErrorKind::Geometry {
                            identifier: Some(geometry.identifier.clone()),
                            message: diagnostic.to_string(),
                        },
                    }),
            );
        }
        for (path, bytes) in sorted_under(files, DATA_DIRECTORY) {
            match read_data_file(path, bytes, files, &by_identifier) {
                Ok((block, entity)) => {
                    models.blocks.insert(block, entity);
                }
                Err(kind) => models.errors.push(BlockEntityError {
                    path: path.to_owned(),
                    kind,
                }),
            }
        }
        models
    }

    /// The content errors found while reading the files.
    pub fn errors(&self) -> &[BlockEntityError] {
        &self.errors
    }

    /// The faces of the drawn layers of the state of `block` with the property values
    /// `state`, in block space, in the order of the layers. The list is empty when no layer is
    /// drawn. A state that names no value of a property of the block is an error.
    pub fn bake(
        &self,
        block: &str,
        state: &[(&str, &str)],
    ) -> Result<Vec<ShapeQuad<'_>>, BakeError> {
        let mut quads = Vec::new();
        let Some(entity) = self.blocks.get(block) else {
            return Ok(quads);
        };
        let not_a_state = || BakeError::NotAState {
            state: format!("{block}{state:?}"),
        };
        let indices: Vec<u8> = entity
            .table
            .state(state)
            .ok_or_else(not_a_state)?
            .into_iter()
            .map(|index| u8::try_from(index).map_err(|_| not_a_state()))
            .collect::<Result<_, _>>()?;
        let mut inputs = StateInputs::new(&entity.properties);
        inputs.select(&indices);
        let mut vm = ScalarVm::new();
        for layer in &entity.layers {
            let drawn = layer
                .when
                .as_ref()
                .is_none_or(|when| when.matches(&mut vm, &inputs));
            if let Some(geometry) = self.geometries.get(layer.geometry)
                && drawn
            {
                bake_layer(layer, geometry, &mut quads);
            }
        }
        Ok(quads)
    }
}

/// One face of a static block entity shape in block space.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeQuad<'a> {
    /// The corners, counter-clockwise when seen from the side that shows.
    pub positions: [Vec3; 4],
    /// The texture coordinates of the corners in the sprite of `texture`. An entity texture
    /// that is not square fills the top-left part of a square sprite.
    pub uvs: [[f32; 2]; 4],
    /// The texture id, `namespace:path`.
    pub texture: &'a str,
    /// The direction closest to the side of the face that shows.
    pub facing: Direction,
    /// The shade factor of the side that shows.
    pub shade: f32,
    /// A dye id whose colour multiplies the texture.
    pub dye: Option<u8>,
    pub surface: Surface,
}

/// The geometries of the geometry files in path order. `paths` takes the path of each
/// identifier in lower case.
fn read_geometries<'a>(
    files: &'a HashMap<String, Vec<u8>>,
    paths: &mut HashMap<String, &'a str>,
    errors: &mut Vec<BlockEntityError>,
) -> Vec<Geometry> {
    let mut geometries = Vec::new();
    for (path, bytes) in sorted_under(files, GEOMETRY_DIRECTORY) {
        let file = match core::str::from_utf8(bytes) {
            Ok(text) => {
                text.parse::<GeometryFile>()
                    .map_err(|error| BlockEntityErrorKind::Geometry {
                        identifier: None,
                        message: error.to_string(),
                    })
            }
            Err(error) => Err(BlockEntityErrorKind::NotUtf8 {
                message: error.to_string(),
            }),
        };
        let file = match file {
            Ok(file) => file,
            Err(kind) => {
                errors.push(BlockEntityError {
                    path: path.to_owned(),
                    kind,
                });
                continue;
            }
        };
        errors.extend(
            file.diagnostics
                .iter()
                .filter(|diagnostic| is_content_error(diagnostic))
                .map(|diagnostic| BlockEntityError {
                    path: path.to_owned(),
                    kind: BlockEntityErrorKind::Geometry {
                        identifier: None,
                        message: diagnostic.to_string(),
                    },
                }),
        );
        for geometry in file.geometries {
            let key = geometry.identifier.to_ascii_lowercase();
            if paths.insert(key, path).is_some() {
                errors.push(BlockEntityError {
                    path: path.to_owned(),
                    kind: BlockEntityErrorKind::DuplicateGeometry {
                        identifier: geometry.identifier.clone(),
                    },
                });
            }
            geometries.push(geometry);
        }
    }
    geometries
}

/// A diagnostic that drops a geometry or a parent link; the other diagnostics are warnings.
fn is_content_error(diagnostic: &Diagnostic) -> bool {
    matches!(
        diagnostic,
        Diagnostic::RejectedGeometry { .. } | Diagnostic::MissingParent { .. }
    )
}

fn sorted_under<'a>(
    files: &'a HashMap<String, Vec<u8>>,
    directory: &str,
) -> Vec<(&'a str, &'a [u8])> {
    let mut found: Vec<(&str, &[u8])> = files
        .iter()
        .filter(|(path, _)| {
            path.strip_prefix(directory)
                .is_some_and(|rest| rest.starts_with('/') && rest.ends_with(".json"))
        })
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect();
    found.sort_unstable_by_key(|(path, _)| *path);
    found
}

/// The block id of a data file path, `<DATA_DIRECTORY>/<namespace>/<path>.json`.
fn block_of(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix(DATA_DIRECTORY)?
        .strip_prefix('/')?
        .strip_suffix(".json")?
        .split_once('/')
}

fn read_data_file(
    path: &str,
    bytes: &[u8],
    files: &HashMap<String, Vec<u8>>,
    geometries: &HashMap<String, usize>,
) -> Result<(String, StaticBlockEntity), BlockEntityErrorKind> {
    let (namespace, block_path) = block_of(path).ok_or(BlockEntityErrorKind::DataPath)?;
    let block = format!("{namespace}:{block_path}");
    let properties = block_properties(namespace, block_path, files).ok_or_else(|| {
        BlockEntityErrorKind::NoBlockDefinition {
            block: block.clone(),
        }
    })?;
    let file: DataFile =
        serde_json::from_slice(bytes).map_err(|error| BlockEntityErrorKind::Syntax {
            message: error.to_string(),
        })?;
    let mut layers = Vec::with_capacity(file.layers.len());
    for (index, entry) in file.layers.into_iter().enumerate() {
        let layer = read_layer(entry, &properties, geometries)
            .map_err(|cause| BlockEntityErrorKind::Layer { index, cause })?;
        layers.push(layer);
    }
    let table = PropertyTable::of_block(&properties);
    Ok((
        block,
        StaticBlockEntity {
            properties,
            table,
            layers,
        },
    ))
}

fn block_properties(
    namespace: &str,
    path: &str,
    files: &HashMap<String, Vec<u8>>,
) -> Option<BlockProperties> {
    if namespace != "minecraft" {
        return None;
    }
    let bytes = files.get(&format!("{CORPUS_DIRECTORY}/{path}.json"))?;
    let file: BlockDefinitionFile = serde_json::from_slice(bytes).ok()?;
    Some(file.block.description.properties)
}

fn read_layer(
    entry: LayerRecord,
    properties: &BlockProperties,
    geometries: &HashMap<String, usize>,
) -> Result<Layer, LayerError> {
    let when = entry
        .when
        .map(|source| {
            StateCondition::compile(&source, properties).map_err(|error| LayerError::Condition {
                message: error.to_string(),
                source,
            })
        })
        .transpose()?;
    let geometry = *geometries
        .get(&entry.geometry.to_ascii_lowercase())
        .ok_or_else(|| LayerError::NoGeometry {
            name: entry.geometry.clone(),
        })?;
    if !entry.texture.contains(':') {
        return Err(LayerError::TextureId {
            texture: entry.texture,
        });
    }
    if entry.tint.is_some_and(|dye| dye >= DYE_COUNT) {
        return Err(LayerError::Tint { tint: entry.tint });
    }
    let [x, y, z] = entry.placement;
    let placement = Mat4::from_cols_array_2d(&[x, y, z, [0.0, 0.0, 0.0, 1.0]]).transpose();
    Ok(Layer {
        when,
        geometry,
        texture: entry.texture,
        dye: entry.tint,
        surface: entry.surface,
        back_faces: entry.back_faces,
        placement,
    })
}

/// The map from internal space to shape space in blocks: internal space mirrors the shape on
/// X, and shape space has +Y down from a height of 24 model units.
fn shape_from_internal() -> Mat4 {
    Mat4::from_cols(
        Vec4::new(-1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, SHAPE_ORIGIN_HEIGHT, 0.0, MODEL_UNITS_PER_BLOCK),
    ) * (1.0 / MODEL_UNITS_PER_BLOCK)
}

fn bake_layer<'a>(layer: &'a Layer, geometry: &RestGeometry, quads: &mut Vec<ShapeQuad<'a>>) {
    let to_block = Affine3A::from_mat4(layer.placement * shape_from_internal());
    let keeps_winding = Mat3::from(to_block.matrix3).determinant() >= 0.0;
    let texture_size = geometry.baked.texture_size;
    let sprite_scale = texture_size / texture_size.max_element();
    let [front, back] = if keeps_winding {
        [[0, 2, 3, 1], [0, 1, 3, 2]]
    } else {
        [[0, 1, 3, 2], [0, 2, 3, 1]]
    };
    for cube in &geometry.baked.cubes {
        let Some(bone) = geometry.bone_transforms.get(cube.bone) else {
            continue;
        };
        let placed = to_block * *bone;
        let normals = Mat3::from(placed.matrix3).inverse().transpose();
        for face in &cube.faces {
            let normal = (normals * face.normal).normalize_or_zero();
            let corners = face.corners.map(|corner| placed.transform_point3(corner));
            let uvs = face.uvs.map(|uv| uv * sprite_scale);
            quads.push(quad(layer, &corners, &uvs, front, normal));
            if layer.back_faces {
                quads.push(quad(layer, &corners, &uvs, back, -normal));
            }
        }
    }
}

fn quad<'a>(
    layer: &'a Layer,
    corners: &[Vec3; 4],
    uvs: &[Vec2; 4],
    order: [usize; 4],
    normal: Vec3,
) -> ShapeQuad<'a> {
    let pick = |index: usize| {
        let corner = corners.get(index).copied().unwrap_or(Vec3::ZERO);
        let uv = uvs.get(index).copied().unwrap_or(Vec2::ZERO);
        (corner, uv.to_array())
    };
    let picked = order.map(pick);
    ShapeQuad {
        positions: picked.map(|(corner, _)| corner),
        uvs: picked.map(|(_, uv)| uv),
        texture: &layer.texture,
        facing: nearest(normal),
        shade: entity_shade(normal),
        dye: layer.dye,
        surface: layer.surface,
    }
}

fn nearest(normal: Vec3) -> Direction {
    let mut best = Direction::Up;
    let mut best_dot = f32::NEG_INFINITY;
    for direction in Direction::all() {
        let dot = normal.dot(direction.normal().as_vec3());
        if dot > best_dot {
            best_dot = dot;
            best = direction;
        }
    }
    best
}

/// The shade factor of an entity face with this normal, under the light directions of the
/// overworld.
fn entity_shade(normal: Vec3) -> f32 {
    let first = Vec3::new(0.2, 1.0, -0.7).normalize();
    let second = Vec3::new(-0.2, 1.0, 0.7).normalize();
    let light = normal.dot(first).max(0.0) + normal.dot(second).max(0.0);
    (light * 0.6 + 0.4).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(properties: &str) -> Vec<u8> {
        format!(
            r#"{{"format_version": "1.0.0", "minecraft:block": {{"description": {{
                "identifier": "minecraft:test_block", "properties": {properties},
                "protocol_id": 0, "base_state_id": 0, "default_state_id": 0}}}}}}"#
        )
        .into_bytes()
    }

    const GEOMETRY: &str = r#"{"format_version": "1.12.0", "minecraft:geometry": [{
        "description": {"identifier": "geometry.test", "texture_width": 16, "texture_height": 16},
        "bones": [{"name": "body", "cubes": [{"origin": [0, 0, 0], "size": [1, 1, 1]}]}]}]}"#;

    fn layer(when: &str) -> String {
        format!(
            r#"{{"layers": [{{"when": "{when}", "geometry": "geometry.test",
                "texture": "minecraft:entity/test", "surface": "opaque", "back_faces": false,
                "placement": [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]]}}]}}"#
        )
    }

    fn models(data: &str) -> BlockEntityModels {
        let files: HashMap<String, Vec<u8>> = [
            (
                format!("{CORPUS_DIRECTORY}/test_block.json"),
                definition(r#"{"facing": ["north", "south"]}"#),
            ),
            (
                format!("{GEOMETRY_DIRECTORY}/test.json"),
                GEOMETRY.as_bytes().to_vec(),
            ),
            (
                format!("{DATA_DIRECTORY}/minecraft/test_block.json"),
                data.as_bytes().to_vec(),
            ),
        ]
        .into_iter()
        .collect();
        BlockEntityModels::from_files(&files)
    }

    #[test]
    fn a_layer_draws_only_in_the_states_where_its_condition_holds() {
        let models = models(&layer("q.block_state('facing') == 'north'"));
        assert_eq!(models.errors(), []);
        let north = models
            .bake("minecraft:test_block", &[("facing", "north")])
            .unwrap();
        assert_eq!(north.len(), 6);
        assert_eq!(
            models.bake("minecraft:test_block", &[("facing", "south")]),
            Ok(Vec::new())
        );
    }

    #[test]
    fn a_condition_on_a_missing_property_is_a_content_error_that_draws_nothing() {
        let models = models(&layer("q.block_state('open') == true"));
        let [error] = models.errors() else {
            panic!("one content error expected, got {:?}", models.errors());
        };
        assert_eq!(
            error.path,
            "mcrs/block_entity_model/minecraft/test_block.json"
        );
        assert_eq!(
            models.bake("minecraft:test_block", &[("facing", "north")]),
            Ok(Vec::new())
        );
    }
}
