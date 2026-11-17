use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeometryError {
    MissingField { field: &'static str },
    InvalidField { field: &'static str },
    NoBones,
    TooManyBones { count: usize },
    TooManyCubes { count: usize },
    TooDeep { bone: String },
    DuplicateBone { name: String },
    ParentCycle { bone: String },
    MissingParentGeometry { parent: String },
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
