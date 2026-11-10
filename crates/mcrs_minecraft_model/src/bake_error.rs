use std::error::Error;
use std::fmt;

use mcrs_minecraft_model_file::face_bake_error::FaceBakeError;

use crate::sprite_error::SpriteError;

/// Why the model of one block state does not bake.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BakeError {
    /// The elements of a model do not bake into faces.
    Face { model: String, cause: FaceBakeError },
    /// The property values name no state of the block.
    NotAState { state: String },
    /// A fluid block has no fluid data.
    NoFluid { block: String },
    /// A baked face names a quad that the baked model lacks. This is a defect of the bake, not of a
    /// pack file.
    PastQuads,
    /// A baked quad names a sprite that the table of the baked model lacks. This is a defect of
    /// the bake, not of a pack file.
    PastSprites,
    /// A sprite of the model cannot be read.
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
