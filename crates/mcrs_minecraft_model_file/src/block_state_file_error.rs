use std::error::Error;
use std::fmt;

use mcrs_minecraft_resource::InvalidResourceLocation;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlockStateFileError {
    Syntax {
        message: String,
    },
    NoParts,
    EmptyVariants,
    EmptyMultipart,
    VariantKey {
        key: String,
        cause: Box<BlockStateFileError>,
    },
    TermWithoutEquals {
        term: String,
    },
    UnknownProperty {
        name: String,
    },
    UnknownValue {
        name: String,
        value: String,
    },
    CombinatorBesideMember,
    CombinatorWithoutArray,
    EmptyCondition,
    TestIsArray {
        name: String,
    },
    EmptyAlternative {
        name: String,
        text: String,
    },
    EmptyPart,
    Rotation {
        degrees: i64,
    },
    Weight {
        weight: i64,
    },
    Location(InvalidResourceLocation),
}

impl fmt::Display for BlockStateFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockStateFileError::Syntax { message } => f.write_str(message),
            BlockStateFileError::NoParts => {
                f.write_str("the file has neither `variants` nor `multipart`")
            }
            BlockStateFileError::EmptyVariants => f.write_str("`variants` is empty"),
            BlockStateFileError::EmptyMultipart => f.write_str("`multipart` is empty"),
            BlockStateFileError::VariantKey { key, cause } => {
                write!(f, "variant key `{key}`: {cause}")
            }
            BlockStateFileError::TermWithoutEquals { term } => {
                write!(f, "the term `{term}` has no `=`")
            }
            BlockStateFileError::UnknownProperty { name } => {
                write!(f, "unknown property `{name}`")
            }
            BlockStateFileError::UnknownValue { name, value } => {
                write!(f, "unknown value `{value}` of `{name}`")
            }
            BlockStateFileError::CombinatorBesideMember => {
                f.write_str("`AND` or `OR` beside another member")
            }
            BlockStateFileError::CombinatorWithoutArray => {
                f.write_str("`AND` or `OR` without an array")
            }
            BlockStateFileError::EmptyCondition => f.write_str("an empty condition"),
            BlockStateFileError::TestIsArray { name } => write!(f, "`{name}` holds an array"),
            BlockStateFileError::EmptyAlternative { name, text } => {
                write!(f, "an empty alternative in `{name}: {text}`")
            }
            BlockStateFileError::EmptyPart => f.write_str("an empty part"),
            BlockStateFileError::Rotation { degrees } => {
                write!(f, "the rotation {degrees} is no quarter turn")
            }
            BlockStateFileError::Weight { weight } => {
                write!(f, "the weight {weight} is below 1")
            }
            BlockStateFileError::Location(error) => write!(
                f,
                "the resource location `{}`: {}",
                error.input, error.reason
            ),
        }
    }
}

impl Error for BlockStateFileError {}

impl From<InvalidResourceLocation> for BlockStateFileError {
    fn from(error: InvalidResourceLocation) -> Self {
        BlockStateFileError::Location(error)
    }
}
