use core::fmt;

use crate::format_version::FormatVersion;
use crate::geometry_error::GeometryError;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Diagnostic {
    UnknownField {
        field: String,
    },
    NewerFormat {
        version: FormatVersion,
    },
    IgnoredBindPoseRotation {
        bone: String,
    },
    CubePivotInOldFormat {
        geometry: String,
    },
    IgnoredMaterialInstance,
    IgnoredMesh {
        bone: String,
    },
    InvalidUvRotation {
        geometry: String,
    },
    NegativeSize {
        geometry: String,
        bone: String,
    },
    MissingParent {
        bone: String,
        parent: String,
    },
    DuplicateLocator {
        geometry: String,
        locator: String,
    },
    DefaultTextureSize {
        geometry: String,
    },
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
