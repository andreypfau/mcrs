use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ModelFileError {
    Syntax { message: String },
    GuiLight { value: String },
    TransformArity { count: usize },
    CornerRange { corner: [f64; 3] },
    NoFace,
    FaceKey { key: String },
    ShadeDirection { name: String },
    LightEmission { level: i64 },
    UvArity { count: usize },
    FaceRotation { degrees: i64 },
    RotationIncomplete,
    RotationAxis { axis: String },
    RotationEmpty,
}

impl fmt::Display for ModelFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelFileError::Syntax { message } => f.write_str(message),
            ModelFileError::GuiLight { value } => write!(f, "unknown gui_light `{value}`"),
            ModelFileError::TransformArity { count } => {
                write!(f, "a transform array of {count} numbers")
            }
            ModelFileError::CornerRange { corner } => {
                write!(f, "the corner {corner:?} is outside -16 to 32")
            }
            ModelFileError::NoFace => f.write_str("an element with no face"),
            ModelFileError::FaceKey { key } => write!(f, "the face key `{key}`"),
            ModelFileError::ShadeDirection { name } => {
                write!(f, "the shade_direction_override `{name}`")
            }
            ModelFileError::LightEmission { level } => write!(f, "the light_emission {level}"),
            ModelFileError::UvArity { count } => write!(f, "a uv of {count} numbers"),
            ModelFileError::FaceRotation { degrees } => write!(f, "the face rotation {degrees}"),
            ModelFileError::RotationIncomplete => {
                f.write_str("an axis rotation needs both `axis` and `angle`")
            }
            ModelFileError::RotationAxis { axis } => write!(f, "the rotation axis `{axis}`"),
            ModelFileError::RotationEmpty => f.write_str("a rotation with no axis and no angle"),
        }
    }
}

impl Error for ModelFileError {}
