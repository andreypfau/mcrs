use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::{AttributeType, AttributeValue, default_value, info};
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use crate::float_provider::EnvironmentNumber;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EnvironmentRead<T> {
    pub(crate) attribute: EnvironmentAttribute,
    pub(crate) positional: bool,
    pub(crate) default: T,
}

pub(crate) fn int_environment<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<EnvironmentRead<i32>, D::Error> {
    let attribute = EnvironmentAttribute::deserialize(deserializer)?;
    match (info(attribute).kind, default_value(attribute)) {
        (AttributeType::Integer, AttributeValue::Integer(default)) => Ok(read(attribute, default)),
        _ => Err(not_a_number()),
    }
}

pub(crate) fn float_environment<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<EnvironmentRead<EnvironmentNumber>, D::Error> {
    let attribute = EnvironmentAttribute::deserialize(deserializer)?;
    let default = match (info(attribute).kind, default_value(attribute)) {
        (AttributeType::Integer, AttributeValue::Integer(default)) => {
            EnvironmentNumber::Int(default)
        }
        (AttributeType::Float | AttributeType::AngleDegrees, AttributeValue::Float(default)) => {
            EnvironmentNumber::Float(default)
        }
        _ => return Err(not_a_number()),
    };
    Ok(read(attribute, default))
}

fn read<T>(attribute: EnvironmentAttribute, default: T) -> EnvironmentRead<T> {
    EnvironmentRead {
        attribute,
        positional: info(attribute).positional,
        default,
    }
}

fn not_a_number<E: Error>() -> E {
    E::custom("the attribute value does not convert to the provider number")
}
