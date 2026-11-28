use std::error::Error;
use std::fmt;

use mcrs_minecraft_model_file::face_bake_error::FaceBakeError;

use crate::sprite_error::SpriteError;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BakeError {
    Face { model: String, cause: FaceBakeError },
    NotAState { state: String },
    NoFluid { block: String },
    PastQuads,
    PastSprites,
    Sprite(SpriteError),
}

impl fmt::Display for BakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BakeError::Face { model, cause } => write!(f, "{model}: {cause}"),
            BakeError::NotAState { state } => write!(f, "{state} is no state of its block"),
            BakeError::NoFluid { block } => write!(f, "{block} states no fluid"),
            BakeError::PastQuads => f.write_str("a face names a quad past the baked model"),
            BakeError::PastSprites => {
                f.write_str("a quad names a sprite past the baked model's table")
            }
            BakeError::Sprite(error) => error.fmt(f),
        }
    }
}

impl Error for BakeError {}

impl From<SpriteError> for BakeError {
    fn from(error: SpriteError) -> Self {
        BakeError::Sprite(error)
    }
}
