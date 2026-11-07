use core::str::FromStr;

use glam::{Vec2, Vec3};

use crate::diagnostic::Diagnostic;
use crate::format_version::{FormatVersion, Shape};
use crate::geometry::Geometry;
use crate::geometry_error::GeometryError;
use crate::json::{Json, JsonObject};
use crate::load_error::LoadError;

const CURRENT_ROOT_FIELDS: [&str; 2] = ["format_version", "minecraft:geometry"];
const CURRENT_GEOMETRY_FIELDS: [&str; 2] = ["description", "bones"];
const DESCRIPTION_FIELDS: [&str; 6] = [
    "identifier",
    "texture_width",
    "texture_height",
    "visible_bounds_width",
    "visible_bounds_height",
    "visible_bounds_offset",
];
const LEGACY_GEOMETRY_FIELDS: [&str; 6] = [
    "texturewidth",
    "textureheight",
    "visible_bounds_width",
    "visible_bounds_height",
    "visible_bounds_offset",
    "bones",
];
const LEGACY_PREFIX: &str = "geometry.";

/// The geometries of one geometry file, and the diagnostics of the file.
///
/// A geometry with a content error is not in `geometries`; a
/// [`Diagnostic::RejectedGeometry`] names it.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct GeometryFile {
    pub format: FormatVersion,
    pub geometries: Vec<Geometry>,
    pub diagnostics: Vec<Diagnostic>,
}

impl FromStr for GeometryFile {
    type Err = LoadError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let json = Json::parse_lenient(text)?;
        let root = json.as_object().ok_or(LoadError::NotAnObject)?;
        let version_text = root
            .get("format_version")
            .and_then(Json::as_str)
            .ok_or(LoadError::MissingFormatVersion)?;
        let unsupported = || LoadError::UnsupportedFormat {
            version: version_text.to_owned(),
        };
        let format: FormatVersion = version_text.parse().map_err(|_| unsupported())?;
        let shape = format.shape().ok_or_else(unsupported)?;
        let mut reader = Reader::new(format, shape);
        if format > FormatVersion::NEWEST_KNOWN {
            reader
                .diagnostics
                .push(Diagnostic::NewerFormat { version: format });
        }
        let geometries = match shape {
            Shape::Current => reader.read_current(root)?,
            Shape::Legacy => reader.read_legacy(root),
        };
        Ok(GeometryFile {
            format,
            geometries,
            diagnostics: reader.diagnostics,
        })
    }
}

/// The state of the reading of one file.
pub(crate) struct Reader {
    pub(crate) format: FormatVersion,
    pub(crate) shape: Shape,
    pub(crate) diagnostics: Vec<Diagnostic>,
    logged_fields: Vec<String>,
    logged_material_instance: bool,
    logged_cube_pivot: bool,
    pub(crate) logged_mesh: bool,
}

impl Reader {
    fn new(format: FormatVersion, shape: Shape) -> Self {
        Self {
            format,
            shape,
            diagnostics: Vec::new(),
            logged_fields: Vec::new(),
            logged_material_instance: false,
            logged_cube_pivot: false,
            logged_mesh: false,
        }
    }

    fn read_current(&mut self, root: &JsonObject) -> Result<Vec<Geometry>, LoadError> {
        self.check_fields(root, &CURRENT_ROOT_FIELDS);
        let entries = root
            .get("minecraft:geometry")
            .and_then(Json::as_array)
            .ok_or(LoadError::MissingGeometries)?;
        let mut geometries = Vec::with_capacity(entries.len());
        for entry in entries {
            match self.read_current_geometry(entry) {
                Ok(geometry) => geometries.push(geometry),
                Err(rejected) => self.diagnostics.push(rejected),
            }
        }
        Ok(geometries)
    }

    fn read_current_geometry(&mut self, entry: &Json) -> Result<Geometry, Diagnostic> {
        let reject = |identifier: Option<&str>, error| Diagnostic::RejectedGeometry {
            identifier: identifier.map(str::to_owned),
            error,
        };
        let object = entry.as_object().ok_or_else(|| {
            reject(
                None,
                GeometryError::InvalidField {
                    field: "minecraft:geometry",
                },
            )
        })?;
        self.check_fields(object, &CURRENT_GEOMETRY_FIELDS);
        let description = object
            .get("description")
            .and_then(Json::as_object)
            .ok_or_else(|| {
                reject(
                    None,
                    GeometryError::MissingField {
                        field: "description",
                    },
                )
            })?;
        self.check_fields(description, &DESCRIPTION_FIELDS);
        let identifier = string(description, "identifier")
            .map_err(|error| reject(None, error))?
            .ok_or_else(|| {
                reject(
                    None,
                    GeometryError::MissingField {
                        field: "identifier",
                    },
                )
            })?;
        let mut geometry = Geometry::new(identifier.to_owned(), self.format);
        self.read_description(
            &mut geometry,
            description,
            ["texture_width", "texture_height"],
        )
        .and_then(|()| self.read_bones(&mut geometry, object))
        .map_err(|error| reject(Some(identifier), error))?;
        Ok(geometry)
    }

    fn read_legacy(&mut self, root: &JsonObject) -> Vec<Geometry> {
        let mut geometries = Vec::new();
        for (key, value) in root.iter() {
            if key == "format_version" {
                continue;
            }
            if !key.starts_with(LEGACY_PREFIX) {
                self.log_field(key);
                continue;
            }
            let (identifier, parent) = match key.split_once(':') {
                Some((identifier, parent)) => (identifier, Some(parent)),
                None => (key, None),
            };
            match self.read_legacy_geometry(identifier, parent, value) {
                Ok(geometry) => geometries.push(geometry),
                Err(error) => self.diagnostics.push(Diagnostic::RejectedGeometry {
                    identifier: Some(identifier.to_owned()),
                    error,
                }),
            }
        }
        geometries
    }

    fn read_legacy_geometry(
        &mut self,
        identifier: &str,
        parent: Option<&str>,
        value: &Json,
    ) -> Result<Geometry, GeometryError> {
        let object = value
            .as_object()
            .ok_or(GeometryError::InvalidField { field: "geometry" })?;
        self.check_fields(object, &LEGACY_GEOMETRY_FIELDS);
        let mut geometry = Geometry::new(identifier.to_owned(), self.format);
        geometry.parent = parent.map(str::to_owned);
        self.read_description(&mut geometry, object, ["texturewidth", "textureheight"])?;
        self.read_bones(&mut geometry, object)?;
        Ok(geometry)
    }

    fn read_description(
        &mut self,
        geometry: &mut Geometry,
        object: &JsonObject,
        [width_field, height_field]: [&'static str; 2],
    ) -> Result<(), GeometryError> {
        let positive = |value: Option<f32>| value.filter(|value| *value > 0.0);
        geometry.texture_width = positive(number(object, width_field)?);
        geometry.texture_height = positive(number(object, height_field)?);
        geometry.visible_bounds_width = number(object, "visible_bounds_width")?;
        geometry.visible_bounds_height = number(object, "visible_bounds_height")?;
        geometry.visible_bounds_offset = vec3(object, "visible_bounds_offset")?;
        Ok(())
    }

    /// Logs each field of `object` that is not in `known`.
    pub(crate) fn check_fields(&mut self, object: &JsonObject, known: &[&str]) {
        for (key, _) in object.iter() {
            if !known.contains(&key) {
                self.log_field(key);
            }
        }
    }

    pub(crate) fn log_field(&mut self, field: &str) {
        if self.logged_fields.iter().any(|logged| logged == field) {
            return;
        }
        self.logged_fields.push(field.to_owned());
        self.diagnostics.push(Diagnostic::UnknownField {
            field: field.to_owned(),
        });
    }

    pub(crate) fn log_material_instance(&mut self) {
        if !self.logged_material_instance {
            self.logged_material_instance = true;
            self.diagnostics.push(Diagnostic::IgnoredMaterialInstance);
        }
    }

    pub(crate) fn log_mesh(&mut self, bone: &str) {
        if !self.logged_mesh {
            self.logged_mesh = true;
            self.diagnostics.push(Diagnostic::IgnoredMesh {
                bone: bone.to_owned(),
            });
        }
    }

    pub(crate) fn log_cube_pivot(&mut self, geometry: &str) {
        if !self.logged_cube_pivot {
            self.logged_cube_pivot = true;
            self.diagnostics.push(Diagnostic::CubePivotInOldFormat {
                geometry: geometry.to_owned(),
            });
        }
    }
}

pub(crate) fn string<'a>(
    object: &'a JsonObject,
    field: &'static str,
) -> Result<Option<&'a str>, GeometryError> {
    object
        .get(field)
        .map(|value| value.as_str().ok_or(GeometryError::InvalidField { field }))
        .transpose()
}

pub(crate) fn boolean(
    object: &JsonObject,
    field: &'static str,
) -> Result<Option<bool>, GeometryError> {
    object
        .get(field)
        .map(|value| value.as_bool().ok_or(GeometryError::InvalidField { field }))
        .transpose()
}

pub(crate) fn number(
    object: &JsonObject,
    field: &'static str,
) -> Result<Option<f32>, GeometryError> {
    object
        .get(field)
        .map(|value| {
            value
                .as_f64()
                .map(|number| number as f32)
                .ok_or(GeometryError::InvalidField { field })
        })
        .transpose()
}

pub(crate) fn vec3(
    object: &JsonObject,
    field: &'static str,
) -> Result<Option<Vec3>, GeometryError> {
    object
        .get(field)
        .map(|value| {
            numbers::<3>(value)
                .map(Vec3::from)
                .ok_or(GeometryError::InvalidField { field })
        })
        .transpose()
}

pub(crate) fn vec2(
    object: &JsonObject,
    field: &'static str,
) -> Result<Option<Vec2>, GeometryError> {
    object
        .get(field)
        .map(|value| {
            numbers::<2>(value)
                .map(Vec2::from)
                .ok_or(GeometryError::InvalidField { field })
        })
        .transpose()
}

/// The numbers of an array of exactly `N` numbers.
pub(crate) fn numbers<const N: usize>(value: &Json) -> Option<[f32; N]> {
    let elements = value.as_array()?;
    if elements.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (slot, element) in out.iter_mut().zip(elements) {
        *slot = element.as_f64()? as f32;
    }
    Some(out)
}
