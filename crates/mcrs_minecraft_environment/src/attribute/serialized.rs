use serde::Serialize;
use serde::de::DeserializeSeed;

use super::modifier::Operation;
use super::spec::{AttributeError, AttributeSpec, AttributeValue};

/// An argument built in code, read the way a data pack's would be.
///
/// Goes through the argument's JSON text rather than a value tree: a tree
/// widens an `f32` to the nearest `f64` and so prints 0.07 as
/// 0.07000000029802322, where the text keeps the digits the `f32` was written
/// with.
pub(super) fn read_argument(
    spec: &AttributeSpec,
    op: Operation,
    argument: &impl Serialize,
) -> Result<AttributeValue, AttributeError> {
    let rejected = |error: serde_json::Error| AttributeError::Rejected(error.to_string());
    let text = serde_json::to_string(argument).map_err(rejected)?;
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    let value = spec
        .argument_seed(op)
        .deserialize(&mut deserializer)
        .map_err(rejected)?;
    deserializer.end().map_err(rejected)?;
    Ok(value)
}
