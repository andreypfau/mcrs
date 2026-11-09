use std::error::Error;
use std::fmt;

use mcrs_minecraft_model_file::pack_files::PackError;

/// Why a sprite, an atlas file or a colormap could not be read from the pack.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpriteError {
    /// A file the sprite needs is not in the pack.
    Pack(PackError),
    /// An atlas file is not JSON of the atlas shape.
    Atlas { id: String, message: String },
    /// An animation file is not JSON of the animation shape.
    Animation { path: String, message: String },
    /// A texture file is not a PNG image.
    Decode { path: String, message: String },
    /// A decoded texture holds no pixel data.
    NoPixels { path: String },
    /// Neither the pack nor an atlas permutation holds the texture.
    NotInPack { path: String },
    /// The registry holds as many sprites as a face can name.
    TooManySprites { id: String, limit: usize },
    /// A texture has frames that are not square.
    FrameNotSquare {
        path: String,
        width: u32,
        height: u32,
    },
    /// An animation names a frame the texture lacks.
    NoFrame { path: String, frame: u32 },
    /// The sprite needs a texture array past the ones the shaders bind.
    TooManyArrays {
        id: String,
        needed: usize,
        limit: usize,
    },
    /// The sprite lands in a texture array the registry lacks.
    NoArray { id: String, index: usize },
    /// Two palettes of a permutation have different sizes.
    PaletteSize { base: usize, target: usize },
    /// A colormap is not 256 by 256 texels.
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
