use core::fmt;

/// A content error that rejects one geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeometryError {
    /// A required field is absent.
    MissingField { field: &'static str },
    /// A field has a value of the wrong type or range.
    InvalidField { field: &'static str },
    /// The geometry has no bone.
    NoBones,
    /// The geometry has more than [`crate::MAX_BONES`] bones.
    TooManyBones { count: usize },
    /// The geometry has more than [`crate::MAX_CUBES`] cubes.
    TooManyCubes { count: usize },
    /// The chain from a root bone to this bone has more than [`crate::MAX_DEPTH`] bones.
    TooDeep { bone: String },
    /// Two bones have this name, compared without regard to letter case.
    DuplicateBone { name: String },
    /// The chain of parents of this bone returns to it.
    ParentCycle { bone: String },
    /// The parent geometry of the legacy shape does not exist.
    MissingParentGeometry { parent: String },
    /// The chain of parent geometries returns to this geometry.
    ParentGeometryCycle,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeometryError::MissingField { field } => write!(formatter, "`{field}` is required"),
            GeometryError::InvalidField { field } => {
                write!(formatter, "`{field}` has an invalid value")
            }
            GeometryError::NoBones => formatter.write_str("the geometry has no bone"),
            GeometryError::TooManyBones { count } => {
                write!(formatter, "{count} bones is more than {}", crate::MAX_BONES)
            }
            GeometryError::TooManyCubes { count } => {
                write!(formatter, "{count} cubes is more than {}", crate::MAX_CUBES)
            }
            GeometryError::TooDeep { bone } => write!(
                formatter,
                "the chain of `{bone}` has more than {} bones",
                crate::MAX_DEPTH
            ),
            GeometryError::DuplicateBone { name } => {
                write!(formatter, "two bones are named `{name}`")
            }
            GeometryError::ParentCycle { bone } => {
                write!(formatter, "the parents of `{bone}` form a cycle")
            }
            GeometryError::MissingParentGeometry { parent } => {
                write!(formatter, "the parent geometry `{parent}` does not exist")
            }
            GeometryError::ParentGeometryCycle => {
                formatter.write_str("the parent geometries form a cycle")
            }
        }
    }
}

impl core::error::Error for GeometryError {}
