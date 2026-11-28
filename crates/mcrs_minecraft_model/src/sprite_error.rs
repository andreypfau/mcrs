use std::error::Error;
use std::fmt;

use mcrs_minecraft_model_file::pack_files::PackError;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpriteError {
    Pack(PackError),
    Atlas {
        id: String,
        message: String,
    },
    Animation {
        path: String,
        message: String,
    },
    Decode {
        path: String,
        message: String,
    },
    NoPixels {
        path: String,
    },
    NotInPack {
        path: String,
    },
    TooManySprites {
        id: String,
        limit: usize,
    },
    FrameNotSquare {
        path: String,
        width: u32,
        height: u32,
    },
    NoFrame {
        path: String,
        frame: u32,
    },
    TooManyArrays {
        id: String,
        needed: usize,
        limit: usize,
    },
    NoArray {
        id: String,
        index: usize,
    },
    PaletteSize {
        base: usize,
        target: usize,
    },
    ColormapSize {
        path: String,
        width: u32,
        height: u32,
    },
}

impl fmt::Display for SpriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpriteError::Pack(error) => error.fmt(f),
            SpriteError::Atlas { id, message } => write!(f, "cannot parse atlas {id}: {message}"),
            SpriteError::Animation { path, message } => {
                write!(f, "cannot parse {path}: {message}")
            }
            SpriteError::Decode { path, message } => write!(f, "cannot decode {path}: {message}"),
            SpriteError::NoPixels { path } => write!(f, "{path} decoded without pixel data"),
            SpriteError::NotInPack { path } => write!(f, "{path} is not in the resource pack"),
            SpriteError::TooManySprites { id, limit } => {
                write!(f, "{id} would be sprite {limit}, past what a face can name")
            }
            SpriteError::FrameNotSquare {
                path,
                width,
                height,
            } => write!(
                f,
                "{path} has {width}x{height} frames, and every layer of an array is square"
            ),
            SpriteError::NoFrame { path, frame } => write!(f, "{path} has no frame {frame}"),
            SpriteError::TooManyArrays { id, needed, limit } => write!(
                f,
                "{id} needs a {needed}th texture array, but the shaders bind {limit}"
            ),
            SpriteError::NoArray { id, index } => {
                write!(
                    f,
                    "{id} lands in texture array {index}, past the registry's"
                )
            }
            SpriteError::PaletteSize { base, target } => {
                write!(f, "PaletteMapping has different sizes: {base} != {target}")
            }
            SpriteError::ColormapSize {
                path,
                width,
                height,
            } => write!(
                f,
                "{path} is {width}x{height}, and a colormap is sampled as a 256x256 grid"
            ),
        }
    }
}

impl Error for SpriteError {}

impl From<PackError> for SpriteError {
    fn from(error: PackError) -> Self {
        SpriteError::Pack(error)
    }
}
