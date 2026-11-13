use std::collections::HashMap;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_animation::Rig;
use mcrs_minecraft_entity_model::{
    BakedCube, BakedGeometry, BoneRecord, CuboidRecords, ModelRecord,
};
use mcrs_minecraft_render_controller::{LayerChoice, RenderControllers, matches_bone};

use crate::impostor_frame::{ImpostorFrames, entity_point};
use crate::pose_program::{PoseProgram, PoseProgramError};
use crate::records::{HIDEABLE_ENTRIES, MAX_LAYERS};

/// The palette index of a bone that no animation writes and no render controller hides.
pub const NO_PALETTE: u16 = u16::MAX;
/// The most definitions and texture arrays that the passes hold.
pub const MAX_DEFINITIONS: usize = 16;
pub const MAX_ARRAYS: usize = 8;

/// One client entity definition as the loader gives it.
#[derive(Clone, Copy, Debug)]
pub struct DefinitionSource<'a> {
    pub rig: &'a Rig,
    /// The geometries in the order of the geometry map of the client entity; `None` for a
    /// short name with no baked geometry.
    pub geometries: &'a [Option<BakedGeometry>],
    /// The render controllers whose `part_visibility` patterns can hide bones.
    pub controllers: &'a RenderControllers,
    /// The flip angle of the death tilt, in degrees.
    pub death_flip: f32,
}

/// An entity texture: the identifier `ns:path` and its RGBA pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureImage {
    pub identifier: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The 16-byte record of one texture in the `variants` buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct VariantRecord {
    /// The layer in the low half and the texture array in the high half.
    pub array_layer: u32,
    pub uv_scale: [f32; 2],
    /// The byte of the cutout mask of the first cuboid slot.
    pub cutout_first: u32,
}

/// The layers of one `texture_2d_array`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureArray {
    pub side: u32,
    /// RGBA pixels of each layer, `side` by `side`.
    pub layers: Vec<Vec<u8>>,
}

/// The static tables of one definition.
#[derive(Clone, Debug)]
pub struct DefinitionTables {
    pub identifier: String,
    /// The global index of the model of the geometry short name 0.
    pub first_model: u32,
    /// For each geometry short name, its global model, or `None` when it has no geometry.
    pub models: Vec<Option<u16>>,
    pub palette: Vec<String>,
    /// For each layer of the render controllers, the palette entries below
    /// [`HIDEABLE_ENTRIES`] that each `part_visibility` pattern matches, as a hidden mask.
    pub part_masks: Vec<Vec<u16>>,
    /// The lanes of one mob in the expand pass: the cuboids of its largest layers.
    pub expand_lanes: u32,
    /// The bones of one mob with its largest layers.
    pub max_bones: u32,
    pub death_flip: f32,
    pub program: Result<PoseProgram, PoseProgramError>,
    /// The height of the entity box at the scale 1, in blocks.
    pub box_height: f32,
    /// The size of a baby relative to the adult.
    pub baby_scale: f32,
}

impl DefinitionTables {
    /// The palette entry of a bone name, without regard to letter case.
    #[must_use]
    pub fn palette_entry(&self, name: &str) -> Option<usize> {
        self.palette
            .iter()
            .position(|entry| entry.eq_ignore_ascii_case(name))
    }

    /// The hidden mask of the state record for the bones that the patterns of `choice` hide.
    /// A later pattern overrides an earlier one for the bones that both match.
    #[must_use]
    pub fn hidden_mask(&self, choice: &LayerChoice) -> u16 {
        let Some(masks) = self.part_masks.get(choice.entry) else {
            return 0;
        };
        let mut hidden = 0u16;
        for (index, mask) in masks.iter().enumerate() {
            if choice.shows_part(index) {
                hidden &= !mask;
            } else {
                hidden |= mask;
            }
        }
        hidden
    }
}

/// The static data of the entity passes: the model tables of each definition, the pose
/// programs and the textures.
#[derive(Clone, Debug, Default)]
pub struct EntityLibrary {
    pub definitions: Vec<DefinitionTables>,
    pub models: Vec<ModelRecord>,
    pub bones: Vec<BoneRecord>,
    /// The 32-byte cuboid slots of all models.
    pub cuboids: Vec<u8>,
    pub variants: Vec<VariantRecord>,
    pub cutout: Vec<u8>,
    /// The transform of each bone of `bones` at rest, as three rows, in model units.
    pub rest_bones: Vec<[[f32; 4]; 3]>,
    /// The views of the impostor images of each model of `models`. The models of one
    /// definition share the depth radius, so the images of its layers compare their depths.
    pub impostor_frames: Vec<ImpostorFrames>,
    pub arrays: Vec<TextureArray>,
    /// The variant of each texture identifier.
    pub textures: HashMap<String, u16>,
    /// The variants whose texels a frame can write: the skin layers. They have no impostor.
    pub skin_variants: Range<u16>,
    /// The diagnostics of the build.
    pub diagnostics: Vec<String>,
}

impl EntityLibrary {
    /// Builds the tables. `layer_limit` is the most layers of one texture array.
    #[must_use]
    pub fn build(
        sources: &[DefinitionSource<'_>],
        textures: &[TextureImage],
        layer_limit: u32,
    ) -> Self {
        let mut library = Self::default();
        let mut baked: Vec<&BakedGeometry> = Vec::new();
        for source in sources.iter().take(MAX_DEFINITIONS) {
            let layers = source.controllers.layers.len();
            if layers > MAX_LAYERS {
                library.diagnostics.push(format!(
                    "{}: {layers} render controller layers; the entity passes draw the first {MAX_LAYERS}",
                    source.rig.identifier
                ));
            }
            let definition = library.add_definition(source, &mut baked);
            library.definitions.push(definition);
        }
        if sources.len() > MAX_DEFINITIONS {
            library.diagnostics.push(format!(
                "{} client entity definitions; the entity passes draw the first {MAX_DEFINITIONS}",
                sources.len()
            ));
        }
        library.add_textures(textures, &baked, layer_limit.max(1));
        library
    }

    fn add_definition<'a>(
        &mut self,
        source: &DefinitionSource<'a>,
        baked: &mut Vec<&'a BakedGeometry>,
    ) -> DefinitionTables {
        let rig = source.rig;
        let mut palette: Vec<String> = Vec::new();
        for geometry in source.geometries.iter().flatten() {
            for bone in &geometry.bones {
                let hideable = part_patterns(source.controllers)
                    .any(|pattern| pattern != "*" && matches_bone(pattern, &bone.name));
                if hideable
                    && !palette
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(&bone.name))
                {
                    palette.push(bone.name.to_ascii_lowercase());
                }
            }
        }
        if palette.len() > HIDEABLE_ENTRIES {
            self.diagnostics.push(format!(
                "{}: more than {HIDEABLE_ENTRIES} bones can be hidden; the rest always draw",
                rig.identifier
            ));
        }
        for bone in &rig.bones {
            if !palette.iter().any(|name| name.eq_ignore_ascii_case(bone)) {
                palette.push(bone.clone());
            }
        }
        let palette_of_bone: Vec<usize> = rig
            .bones
            .iter()
            .map(|bone| {
                palette
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(bone))
                    .unwrap_or(0)
            })
            .collect();
        let rest = rest_rotations(source.geometries, &palette);
        let first_model = self.models.len() as u32;
        let mut models = Vec::with_capacity(source.geometries.len());
        let mut cuboid_counts = Vec::new();
        let mut bone_counts = Vec::new();
        let mut rest_points = Vec::new();
        let first_frames = self.impostor_frames.len();
        for geometry in source.geometries {
            let Some(geometry) = geometry else {
                models.push(None);
                continue;
            };
            match self.add_model(geometry, &palette) {
                Ok(model) => {
                    models.push(Some(model));
                    baked.push(geometry);
                    cuboid_counts.push(geometry.cubes.len() as u32);
                    bone_counts.push(geometry.bones.len() as u32);
                    let points: Vec<Vec3> = rest_corners(geometry).collect();
                    self.impostor_frames
                        .push(ImpostorFrames::enclosing(&points));
                    rest_points.extend(points);
                }
                Err(message) => {
                    self.diagnostics.push(format!(
                        "{} / {}: {message}",
                        rig.identifier, geometry.identifier
                    ));
                    models.push(None);
                }
            }
        }
        let largest = |mut counts: Vec<u32>| {
            counts.sort_unstable_by(|a, b| b.cmp(a));
            counts.iter().take(MAX_LAYERS).sum::<u32>().max(1)
        };
        let program = PoseProgram::generate(rig, &palette_of_bone, &rest);
        if let Err(error) = &program {
            self.diagnostics
                .push(format!("{}: no GPU pose: {error}", rig.identifier));
        }
        let depth_radius = ImpostorFrames::enclosing(&rest_points).depth_radius;
        for frames in self.impostor_frames.iter_mut().skip(first_frames) {
            frames.depth_radius = depth_radius;
        }
        let (box_height, baby_scale) = box_size(&rig.identifier, &rest_points);
        DefinitionTables {
            identifier: rig.identifier.clone(),
            first_model,
            models,
            part_masks: part_masks(source.controllers, &palette),
            palette,
            expand_lanes: largest(cuboid_counts),
            max_bones: largest(bone_counts),
            death_flip: source.death_flip,
            program,
            box_height,
            baby_scale,
        }
    }

    fn add_model(&mut self, geometry: &BakedGeometry, palette: &[String]) -> Result<u16, String> {
        let index = u16::try_from(self.models.len()).map_err(|_| "too many models".to_owned())?;
        let records = CuboidRecords::new(geometry).map_err(|error| error.to_string())?;
        let first_bone = self.bones.len() as u32;
        let first_cuboid = (self.cuboids.len() / 32) as u32;
        for transform in geometry.model_transforms(&[]) {
            let linear = transform.matrix3;
            let translation = transform.translation;
            self.rest_bones.push([0, 1, 2].map(|row| {
                [
                    linear.x_axis[row],
                    linear.y_axis[row],
                    linear.z_axis[row],
                    translation[row],
                ]
            }));
        }
        for bone in &geometry.bones {
            let entry = palette
                .iter()
                .position(|name| name.eq_ignore_ascii_case(&bone.name))
                .and_then(|entry| u16::try_from(entry).ok())
                .unwrap_or(NO_PALETTE);
            self.bones.push(BoneRecord::new(bone, entry));
        }
        records.extend_bytes(&mut self.cuboids);
        let mut record = ModelRecord::new(geometry, first_bone, first_cuboid);
        record.cuboid_count = records.cuboids.len() as u32;
        self.models.push(record);
        Ok(index)
    }

    fn add_textures(
        &mut self,
        textures: &[TextureImage],
        baked: &[&BakedGeometry],
        layer_limit: u32,
    ) {
        let slots = self.cuboids.len() / 32;
        for image in textures {
            let side = image.width.max(image.height).max(1);
            let array =
                match self.arrays.iter().position(|array| {
                    array.side == side && (array.layers.len() as u32) < layer_limit
                }) {
                    Some(array) => array,
                    None if self.arrays.len() < MAX_ARRAYS => {
                        self.arrays.push(TextureArray {
                            side,
                            layers: Vec::new(),
                        });
                        self.arrays.len() - 1
                    }
                    None => {
                        self.diagnostics.push(format!(
                            "{}: more than {MAX_ARRAYS} entity texture arrays",
                            image.identifier
                        ));
                        continue;
                    }
                };
            let Some(target) = self.arrays.get_mut(array) else {
                continue;
            };
            let layer = target.layers.len() as u32;
            target.layers.push(pad(image, side));
            let Ok(variant) = u16::try_from(self.variants.len()) else {
                continue;
            };
            let cutout_first = self.cutout.len() as u32;
            self.cutout.resize(self.cutout.len() + slots, 0);
            for (model, geometry) in self.models.iter().zip(baked) {
                for (index, cube) in geometry.cubes.iter().enumerate() {
                    let at = cutout_first as usize + model.first_cuboid as usize + index;
                    if let Some(byte) = self.cutout.get_mut(at) {
                        *byte = cutout_faces(image, cube);
                    }
                }
            }
            self.variants.push(VariantRecord {
                array_layer: layer | ((array as u32) << 16),
                uv_scale: [
                    image.width as f32 / side as f32,
                    image.height as f32 / side as f32,
                ],
                cutout_first,
            });
            self.textures.insert(image.identifier.clone(), variant);
        }
        while !self.cutout.len().is_multiple_of(4) {
            self.cutout.push(0);
        }
    }
}

/// The box height and the baby scale of the entity type `identifier`. A type with no box takes
/// the height of the rest pose of its models.
fn box_size(identifier: &str, rest_points: &[Vec3]) -> (f32, f32) {
    let traits = EntityType::read(identifier).and_then(EntityTypeData::of);
    let (low, high) = rest_points
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), point| {
            (low.min(point.y), high.max(point.y))
        });
    let box_height = match traits {
        Some(traits) if traits.height > 0.0 => traits.height,
        _ if low < high => high - low,
        _ => 1.0,
    };
    (
        box_height,
        traits.and_then(|traits| traits.baby_scale).unwrap_or(1.0),
    )
}

/// The corners of the faces of `geometry` at rest, in entity space at the body yaw 0.
/// The file rotation of each palette bone in the first geometry that has it.
fn rest_rotations(geometries: &[Option<BakedGeometry>], palette: &[String]) -> Vec<Vec3> {
    palette
        .iter()
        .map(|name| {
            geometries
                .iter()
                .flatten()
                .find_map(|geometry| {
                    geometry
                        .bones
                        .iter()
                        .find(|bone| bone.name.eq_ignore_ascii_case(name))
                        .map(|bone| bone.file_rotation)
                })
                .unwrap_or(Vec3::ZERO)
        })
        .collect()
}

fn rest_corners(geometry: &BakedGeometry) -> impl Iterator<Item = Vec3> + '_ {
    let transforms = geometry.model_transforms(&[]);
    geometry.cubes.iter().flat_map(move |cube| {
        let transform = transforms.get(cube.bone).copied().unwrap_or_default();
        cube.faces
            .iter()
            .flat_map(|face| face.corners)
            .map(move |corner| entity_point(transform.transform_point3(corner)))
    })
}

/// The image in the top left corner of a transparent square of `side`.
fn pad(image: &TextureImage, side: u32) -> Vec<u8> {
    let side = side as usize;
    let row = image.width as usize * 4;
    let mut out = vec![0; side * side * 4];
    for (target, line) in out
        .chunks_exact_mut(side * 4)
        .zip(image.rgba.chunks_exact(row.max(4)))
    {
        if let (Some(start), Some(line)) = (target.get_mut(..row), line.get(..row)) {
            start.copy_from_slice(line);
        }
    }
    out
}

/// The faces of `cube` with a texel of `image` under the cutout limit, as a face mask.
fn cutout_faces(image: &TextureImage, cube: &BakedCube) -> u8 {
    cube.faces
        .iter()
        .filter(|face| has_cutout(image, face.uvs))
        .fold(0, |mask, face| mask | face.face.mask_bit())
}

/// Whether a texel under the face rectangle `uvs` has an alpha under the cutout limit.
fn has_cutout(image: &TextureImage, uvs: [glam::Vec2; 4]) -> bool {
    let (width, height) = (image.width as f32, image.height as f32);
    let min = uvs.iter().fold(glam::Vec2::INFINITY, |a, b| a.min(*b));
    let max = uvs.iter().fold(glam::Vec2::NEG_INFINITY, |a, b| a.max(*b));
    let x0 = (min.x * width).floor().max(0.0) as u32;
    let y0 = (min.y * height).floor().max(0.0) as u32;
    let x1 = ((max.x * width).ceil() as u32).min(image.width);
    let y1 = ((max.y * height).ceil() as u32).min(image.height);
    for y in y0..y1 {
        for x in x0..x1 {
            let alpha = image
                .rgba
                .get(((y * image.width + x) * 4 + 3) as usize)
                .copied()
                .unwrap_or(0);
            if f32::from(alpha) / 255.0 < 0.1 {
                return true;
            }
        }
    }
    false
}

fn part_patterns(controllers: &RenderControllers) -> impl Iterator<Item = &str> {
    controllers
        .layers
        .iter()
        .flat_map(|layer| &layer.part_visibility)
        .map(|(pattern, _)| pattern.as_str())
}

/// The hidden mask of the palette entries that each `part_visibility` pattern of each layer
/// matches.
fn part_masks(controllers: &RenderControllers, palette: &[String]) -> Vec<Vec<u16>> {
    let pattern_mask = |pattern: &str| {
        let mut mask = 0u16;
        for (index, name) in palette.iter().enumerate().take(HIDEABLE_ENTRIES) {
            if matches_bone(pattern, name) {
                mask |= 1 << index;
            }
        }
        mask
    };
    controllers
        .layers
        .iter()
        .map(|layer| {
            layer
                .part_visibility
                .iter()
                .map(|(pattern, _)| pattern_mask(pattern))
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_entity_animation::EntityQuery;
    use mcrs_minecraft_render_controller::RenderQuery;
    use mcrs_molang::{ScalarVm, Value};

    use super::*;
    use crate::entity_pack::EntityPack;

    const ENTITY: &str = r#"{"minecraft:client_entity": {"description": {"identifier": "test:mob",
        "geometry": {"default": "geometry.mob"}, "textures": {"default": "mob"},
        "render_controllers": ["controller.render.mob"]}}}"#;
    const GEOMETRY: &str = r#"{"format_version": "1.12.0", "minecraft:geometry": [{
        "description": {"identifier": "geometry.mob", "texture_width": 16, "texture_height": 16},
        "bones": [{"name": "body", "pivot": [0, 0, 0]}, {"name": "head", "parent": "body", "pivot": [0, 1, 0]}]}]}"#;
    const CONTROLLERS: &str = r#"{"render_controllers": {"controller.render.mob": {
        "geometry": "Geometry.default", "textures": ["Texture.default"],
        "part_visibility": [{"head": false}, {"h*": "q.is_baby"}]}}}"#;

    #[test]
    fn a_later_part_visibility_pattern_overrides_an_earlier_one() {
        let pack = EntityPack::load([
            ("entity/mob.entity.json", ENTITY),
            ("geometry/mob.json", GEOMETRY),
            ("render_controllers/mob.json", CONTROLLERS),
        ]);
        assert!(pack.diagnostics.is_empty(), "{:?}", pack.diagnostics);
        let library = EntityLibrary::build(&pack.sources(), &[], 1);
        let (definition, entity) = pack.entity("test:mob").unwrap();
        let tables = &library.definitions[definition];
        let head = 1 << tables.palette_entry("head").unwrap();
        let mut vm = ScalarVm::new();
        for (baby, hidden) in [(false, head), (true, 0)] {
            let choices = entity.controllers.evaluate(&mut vm, |query| {
                Some(Value::Number(match query {
                    RenderQuery::Entity(EntityQuery::IsBaby) if baby => 1.0,
                    _ => 0.0,
                }))
            });
            assert_eq!(tables.hidden_mask(&choices[0]), hidden, "baby {baby}");
        }
    }
}
