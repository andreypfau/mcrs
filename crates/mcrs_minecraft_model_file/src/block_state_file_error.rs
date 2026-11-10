use std::error::Error;
use std::fmt;

/// Why a blockstate file, or one of its variant keys, is a content error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlockStateFileError {
    /// The file is not JSON of the blockstate shape.
    Syntax { message: String },
    /// The file has neither `variants` nor `multipart`.
    NoParts,
    /// The `variants` object has no member, so no state has a model: the whole file is rejected.
    EmptyVariants,
    /// The `multipart` array has no entry, so no state has a model: the whole file is rejected.
    EmptyMultipart,
    /// A variant key is wrong; the key matches no state.
    VariantKey {
        key: String,
        cause: Box<BlockStateFileError>,
    },
    /// A term of a variant key has no `=`.
    TermWithoutEquals { term: String },
    /// A key or condition names a property the block lacks.
    UnknownProperty { name: String },
    /// A key or condition names a value the property lacks.
    UnknownValue { name: String, value: String },
    /// `AND` or `OR` stands beside another member of a condition.
    CombinatorBesideMember,
    /// `AND` or `OR` holds no array.
    CombinatorWithoutArray,
    /// A `when` condition with no member would match each state by accident: the file is rejected.
    EmptyCondition,
    /// A property test takes one value or alternatives joined by `|`, never an array.
    TestIsArray { name: String },
    /// A property test has an empty alternative.
    EmptyAlternative { name: String, text: String },
    /// A part with no variant has nothing to choose from: the file is rejected.
    EmptyPart,
    /// A variant rotation is no quarter turn.
    Rotation { degrees: i64 },
    /// A variant weight is below 1.
    Weight { weight: i64 },
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
        }
    }
}

impl Error for BlockStateFileError {}
