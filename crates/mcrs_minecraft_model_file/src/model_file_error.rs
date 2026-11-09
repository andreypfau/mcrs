use std::error::Error;
use std::fmt;

/// Why a model file is not a model: a content error, after which the model is absent.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ModelFileError {
    /// The file is not JSON of the model shape.
    Syntax { message: String },
    /// The `gui_light` value is neither `front` nor `side`.
    GuiLight { value: String },
    /// A transform array has other than three numbers.
    TransformArity { count: usize },
    /// A corner of an element lies outside -16 to 32.
    CornerRange { corner: [f64; 3] },
    /// An element has no face.
    NoFace,
    /// A face key is no direction name.
    FaceKey { key: String },
    /// A `shade_direction_override` is no direction name.
    ShadeDirection { name: String },
    /// A `light_emission` lies outside 0 to 15.
    LightEmission { level: i64 },
    /// A face `uv` has other than four numbers.
    UvArity { count: usize },
    /// A face rotation is no multiple of 90 degrees.
    FaceRotation { degrees: i64 },
    /// An axis rotation names only one of `axis` and `angle`.
    RotationIncomplete,
    /// A rotation axis is none of x, y and z.
    RotationAxis { axis: String },
    /// A rotation has neither `axis` and `angle` nor an Euler angle.
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
