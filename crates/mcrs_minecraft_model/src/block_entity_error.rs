use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockEntityError {
    pub path: String,
    pub kind: BlockEntityErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlockEntityErrorKind {
    NotUtf8 {
        message: String,
    },
    Geometry {
        identifier: Option<String>,
        message: String,
    },
    DuplicateGeometry {
        identifier: String,
    },
    DataPath,
    NoBlockDefinition {
        block: String,
    },
    Syntax {
        message: String,
    },
    Layer {
        index: usize,
        cause: LayerError,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayerError {
    Condition { source: String, message: String },
    NoGeometry { name: String },
    TextureId { texture: String },
    Tint { tint: Option<u8> },
}

impl fmt::Display for BlockEntityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.kind)
    }
}

impl fmt::Display for BlockEntityErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockEntityErrorKind::NotUtf8 { message }
            | BlockEntityErrorKind::Syntax { message } => f.write_str(message),
            BlockEntityErrorKind::Geometry {
                identifier: Some(identifier),
                message,
            } => write!(f, "{identifier}: {message}"),
            BlockEntityErrorKind::Geometry {
                identifier: None,
                message,
            } => f.write_str(message),
            BlockEntityErrorKind::DuplicateGeometry { identifier } => {
                write!(f, "the geometry {identifier} is defined again")
            }
            BlockEntityErrorKind::DataPath => {
                f.write_str("the path does not name a block as <namespace>/<path>.json")
            }
            BlockEntityErrorKind::NoBlockDefinition { block } => {
                write!(f, "the block {block} has no block definition")
            }
            BlockEntityErrorKind::Layer { index, cause } => write!(f, "layer {index}: {cause}"),
        }
    }
}

impl fmt::Display for LayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayerError::Condition { source, message } => {
                write!(f, "`when` {source:?}: {message}")
            }
            LayerError::NoGeometry { name } => write!(f, "no geometry {name}"),
            LayerError::TextureId { texture } => write!(f, "the texture {texture} is not ns:path"),
            LayerError::Tint { tint } => write!(f, "the tint {tint:?} is not a dye id"),
        }
    }
}

impl Error for BlockEntityError {}
impl Error for LayerError {}
