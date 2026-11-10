use core::fmt;

use crate::format_version::FormatVersion;
use crate::geometry_error::GeometryError;

/// A warning or a content error that does not reject the file. The loader logs each one with
/// the path of the file.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Diagnostic {
    /// The file has a field that the geometry rules do not name. One per field name and file.
    UnknownField { field: String },
    /// The file has a format version above [`FormatVersion::NEWEST_KNOWN`].
    NewerFormat { version: FormatVersion },
    /// A legacy bone has `bind_pose_rotation`, which the client ignores.
    IgnoredBindPoseRotation { bone: String },
    /// A cube of a `1.12.0` file has a pivot, which the client reads as in `1.14.0`.
    CubePivotInOldFormat { geometry: String },
    /// A face has `material_instance`, which the client ignores. One per file.
    IgnoredMaterialInstance,
    /// A bone has a mesh that the client does not draw yet. One per geometry: `bone` is the
    /// first bone with a mesh.
    IgnoredMesh { bone: String },
    /// A face has a `uv_rotation` that is not 0, 90, 180 or 270; the client uses 0.
    InvalidUvRotation { geometry: String },
    /// A cube has a negative size; the client uses 0 on that axis.
    NegativeSize { geometry: String, bone: String },
    /// A bone names a parent that the geometry does not have; the bone attaches to the origin.
    MissingParent { bone: String, parent: String },
    /// Two locators of the geometry have the same name; the first one in bone table order is
    /// kept.
    DuplicateLocator { geometry: String, locator: String },
    /// The geometry does not set its texture size, or sets a width or a height of 0 or less;
    /// the client uses 16 × 16.
    DefaultTextureSize { geometry: String },
    /// A geometry of the file is rejected.
    RejectedGeometry {
        identifier: Option<String>,
        error: GeometryError,
    },
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Diagnostic::UnknownField { field } => write!(formatter, "unknown field `{field}`"),
            Diagnostic::NewerFormat { version } => write!(
                formatter,
                "format_version {version} is newer than {}",
                FormatVersion::NEWEST_KNOWN
            ),
            Diagnostic::IgnoredBindPoseRotation { bone } => {
                write!(
                    formatter,
                    "`bind_pose_rotation` of bone `{bone}` is ignored"
                )
            }
            Diagnostic::CubePivotInOldFormat { geometry } => write!(
                formatter,
                "{geometry}: a cube pivot of format 1.12.0 is read as in 1.14.0"
            ),
            Diagnostic::IgnoredMaterialInstance => {
                formatter.write_str("`material_instance` is ignored")
            }
            Diagnostic::IgnoredMesh { bone } => {
                write!(formatter, "the mesh of bone `{bone}` is not supported")
            }
            Diagnostic::InvalidUvRotation { geometry } => {
                write!(formatter, "{geometry}: `uv_rotation` is not a quarter turn")
            }
            Diagnostic::NegativeSize { geometry, bone } => write!(
                formatter,
                "{geometry}: a cube of bone `{bone}` has a negative size"
            ),
            Diagnostic::MissingParent { bone, parent } => write!(
                formatter,
                "the parent `{parent}` of bone `{bone}` does not exist"
            ),
            Diagnostic::DuplicateLocator { geometry, locator } => write!(
                formatter,
                "{geometry}: the locator `{locator}` is defined again; the first one is kept"
            ),
            Diagnostic::DefaultTextureSize { geometry } => {
                write!(formatter, "{geometry} has no texture size; 16 × 16 is used")
            }
            Diagnostic::RejectedGeometry { identifier, error } => match identifier {
                Some(identifier) => write!(formatter, "{identifier} is rejected: {error}"),
                None => write!(formatter, "a geometry is rejected: {error}"),
            },
        }
    }
}
