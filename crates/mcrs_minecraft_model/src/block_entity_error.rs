use std::error::Error;
use std::fmt;

/// A defect in one file of the static block entity data. The file is skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockEntityError {
    /// The pack path of the file.
    pub path: String,
    pub kind: BlockEntityErrorKind,
}

/// What is wrong with a file of the static block entity data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlockEntityErrorKind {
    /// A geometry file is not UTF-8 text.
    NotUtf8 { message: String },
    /// A geometry file, or one geometry of it, is rejected.
    Geometry {
        identifier: Option<String>,
        message: String,
    },
    /// Two geometry files define the same geometry.
    DuplicateGeometry { identifier: String },
    /// A data file path does not name a block as `<namespace>/<path>.json`.
    DataPath,
    /// The block of a data file has no block definition.
    NoBlockDefinition { block: String },
    /// A data file is not JSON of the data shape.
    Syntax { message: String },
    /// A layer of a data file is wrong.
    Layer { index: usize, cause: LayerError },
}

/// What is wrong with one layer of a data file.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayerError {
    /// The `when` condition does not compile against the properties of the block.
    Condition { source: String, message: String },
    /// The layer names a geometry that no geometry file defines.
    NoGeometry { name: String },
    /// The texture is not `namespace:path`.
    TextureId { texture: String },
    /// The tint is no dye id.
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
