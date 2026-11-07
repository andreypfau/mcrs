use glam::{Vec2, Vec3};

use crate::cube::{Cube, CubeUv, FaceUv, PerFaceUv, UvRotation};
use crate::diagnostic::Diagnostic;
use crate::face::Face;
use crate::format_version::{FormatVersion, Shape};
use crate::geometry::{Bone, Geometry, Locator};
use crate::geometry_error::GeometryError;
use crate::geometry_file::{Reader, boolean, number, numbers, string, vec2, vec3};
use crate::json::{Json, JsonObject};

const CURRENT_BONE_FIELDS: [&str; 12] = [
    "name",
    "parent",
    "pivot",
    "rotation",
    "mirror",
    "inflate",
    "cubes",
    "locators",
    "binding",
    "render_group_id",
    "texture_meshes",
    "poly_mesh",
];
const LEGACY_BONE_FIELDS: [&str; 11] = [
    "name",
    "parent",
    "pivot",
    "rotation",
    "mirror",
    "inflate",
    "cubes",
    "locators",
    "bind_pose_rotation",
    "neverRender",
    "reset",
];
const CURRENT_CUBE_FIELDS: [&str; 7] = [
    "origin", "size", "rotation", "pivot", "inflate", "mirror", "uv",
];
const LEGACY_CUBE_FIELDS: [&str; 5] = ["origin", "size", "uv", "inflate", "mirror"];
const FACE_UV_FIELDS: [&str; 4] = ["uv", "uv_size", "uv_rotation", "material_instance"];
const LOCATOR_FIELDS: [&str; 3] = ["offset", "rotation", "ignore_inherited_scale"];
const MESH_FIELDS: [&str; 2] = ["texture_meshes", "poly_mesh"];

impl Reader {
    pub(crate) fn read_bones(
        &mut self,
        geometry: &mut Geometry,
        object: &JsonObject,
    ) -> Result<(), GeometryError> {
        let Some(entries) = object.get("bones") else {
            return Ok(());
        };
        let entries = entries
            .as_array()
            .ok_or(GeometryError::InvalidField { field: "bones" })?;
        self.logged_mesh = false;
        for entry in entries {
            let bone = self.read_bone(&geometry.identifier, entry)?;
            geometry.bones.push(bone);
        }
        Ok(())
    }

    fn read_bone(&mut self, geometry: &str, entry: &Json) -> Result<Bone, GeometryError> {
        let object = entry
            .as_object()
            .ok_or(GeometryError::InvalidField { field: "bones" })?;
        let legacy = self.shape == Shape::Legacy;
        self.check_fields(
            object,
            if legacy {
                &LEGACY_BONE_FIELDS
            } else {
                &CURRENT_BONE_FIELDS
            },
        );
        let name = string(object, "name")?.ok_or(GeometryError::MissingField { field: "name" })?;
        let mut bone = Bone::new(name.to_owned());
        bone.parent = string(object, "parent")?.map(str::to_owned);
        bone.pivot = vec3(object, "pivot")?.unwrap_or(Vec3::ZERO);
        bone.rotation = vec3(object, "rotation")?.unwrap_or(Vec3::ZERO);
        bone.mirror = boolean(object, "mirror")?.unwrap_or(false);
        bone.inflate = number(object, "inflate")?.unwrap_or(0.0);
        if legacy {
            bone.never_render = boolean(object, "neverRender")?.unwrap_or(false);
            if object.get("bind_pose_rotation").is_some() {
                self.diagnostics.push(Diagnostic::IgnoredBindPoseRotation {
                    bone: bone.name.clone(),
                });
            }
        } else {
            bone.binding = string(object, "binding")?.map(str::to_owned);
            if MESH_FIELDS.iter().any(|field| object.get(field).is_some()) {
                self.log_mesh(&bone.name);
            }
        }
        if let Some(cubes) = object.get("cubes") {
            let cubes = cubes
                .as_array()
                .ok_or(GeometryError::InvalidField { field: "cubes" })?;
            for cube in cubes {
                let cube = self.read_cube(geometry, &bone.name, cube)?;
                bone.cubes.push(cube);
            }
        }
        if let Some(locators) = object.get("locators") {
            bone.locators = self.read_locators(locators)?;
        }
        Ok(bone)
    }

    fn read_cube(
        &mut self,
        geometry: &str,
        bone: &str,
        entry: &Json,
    ) -> Result<Cube, GeometryError> {
        let object = entry
            .as_object()
            .ok_or(GeometryError::InvalidField { field: "cubes" })?;
        let legacy = self.shape == Shape::Legacy;
        self.check_fields(
            object,
            if legacy {
                &LEGACY_CUBE_FIELDS
            } else {
                &CURRENT_CUBE_FIELDS
            },
        );
        let origin =
            vec3(object, "origin")?.ok_or(GeometryError::MissingField { field: "origin" })?;
        let size = vec3(object, "size")?.ok_or(GeometryError::MissingField { field: "size" })?;
        if size.min_element() < 0.0 {
            self.diagnostics.push(Diagnostic::NegativeSize {
                geometry: geometry.to_owned(),
                bone: bone.to_owned(),
            });
        }
        let uv = match object.get("uv") {
            None => CubeUv::Box(Vec2::ZERO),
            Some(Json::Object(faces)) if !legacy => {
                CubeUv::PerFace(self.read_per_face(geometry, faces)?)
            }
            Some(value) => CubeUv::Box(
                numbers::<2>(value)
                    .map(Vec2::from)
                    .ok_or(GeometryError::InvalidField { field: "uv" })?,
            ),
        };
        let mut cube = Cube::new(origin, size.max(Vec3::ZERO), uv);
        cube.inflate = number(object, "inflate")?;
        cube.mirror = boolean(object, "mirror")?;
        if !legacy {
            cube.rotation = vec3(object, "rotation")?.unwrap_or(Vec3::ZERO);
            cube.pivot = vec3(object, "pivot")?;
            if cube.pivot.is_some() && self.format == FormatVersion::V1_12_0 {
                self.log_cube_pivot(geometry);
            }
        }
        Ok(cube)
    }

    fn read_per_face(
        &mut self,
        geometry: &str,
        faces: &JsonObject,
    ) -> Result<PerFaceUv, GeometryError> {
        let mut per_face = PerFaceUv::default();
        for (key, value) in faces.iter() {
            let Ok(face) = key.parse::<Face>() else {
                self.log_field(key);
                continue;
            };
            let object = value
                .as_object()
                .ok_or(GeometryError::InvalidField { field: "uv" })?;
            self.check_fields(object, &FACE_UV_FIELDS);
            let uv = vec2(object, "uv")?.ok_or(GeometryError::MissingField { field: "uv" })?;
            let mut face_uv = FaceUv::new(uv);
            face_uv.uv_size = vec2(object, "uv_size")?;
            if let Some(degrees) = object.get("uv_rotation") {
                match degrees.as_f64().and_then(UvRotation::from_degrees) {
                    Some(rotation) => face_uv.rotation = rotation,
                    None => self.diagnostics.push(Diagnostic::InvalidUvRotation {
                        geometry: geometry.to_owned(),
                    }),
                }
            }
            if object.get("material_instance").is_some() {
                self.log_material_instance();
            }
            per_face.set(face, face_uv);
        }
        Ok(per_face)
    }

    fn read_locators(&mut self, value: &Json) -> Result<Vec<Locator>, GeometryError> {
        let invalid = GeometryError::InvalidField { field: "locators" };
        let entries = value.as_object().ok_or(invalid.clone())?;
        let mut locators = Vec::with_capacity(entries.len());
        for (name, entry) in entries.iter() {
            if let Some(offset) = numbers::<3>(entry) {
                locators.push(Locator::new(name.to_owned(), Vec3::from(offset)));
                continue;
            }
            let object = match entry {
                Json::Object(object) if self.shape == Shape::Current => object,
                _ => return Err(invalid),
            };
            self.check_fields(object, &LOCATOR_FIELDS);
            let offset =
                vec3(object, "offset")?.ok_or(GeometryError::MissingField { field: "offset" })?;
            let mut locator = Locator::new(name.to_owned(), offset);
            locator.rotation = vec3(object, "rotation")?.unwrap_or(Vec3::ZERO);
            locator.ignore_inherited_scale =
                boolean(object, "ignore_inherited_scale")?.unwrap_or(false);
            locators.push(locator);
        }
        Ok(locators)
    }
}
