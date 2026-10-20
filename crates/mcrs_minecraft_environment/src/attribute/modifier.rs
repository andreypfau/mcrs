use serde::{Deserialize, Serialize};

use super::spec::AttributeValue;

/// The operation a modifier names to compose its argument onto the layer beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Override,
    AlphaBlend,
    Add,
    Subtract,
    Multiply,
    BlendToGray,
    Minimum,
    Maximum,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Append,
    Overlay,
}

#[derive(Debug, thiserror::Error)]
pub enum ModifierError {
    #[error("the {0:?} modifier is not supported")]
    Unsupported(Operation),
}

/// Only `override` composes; its argument replaces the subject.
pub fn apply(
    op: Operation,
    _subject: &AttributeValue,
    argument: &AttributeValue,
) -> Result<AttributeValue, ModifierError> {
    match op {
        Operation::Override => Ok(argument.clone()),
        op => Err(ModifierError::Unsupported(op)),
    }
}
