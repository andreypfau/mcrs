use serde::{Deserialize, Serialize};

use crate::Direction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rotation {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "clockwise_90")]
    Clockwise90,
    #[serde(rename = "180")]
    Clockwise180,
    #[serde(rename = "counterclockwise_90")]
    Counterclockwise90,
}

impl Rotation {
    /// `Rotation.values()`, the order `nextInt(4)` indexes.
    pub const ALL: [Rotation; 4] = [
        Rotation::None,
        Rotation::Clockwise90,
        Rotation::Clockwise180,
        Rotation::Counterclockwise90,
    ];

    pub const fn legacy_name(self) -> &'static str {
        match self {
            Rotation::None => "NONE",
            Rotation::Clockwise90 => "CLOCKWISE_90",
            Rotation::Clockwise180 => "CLOCKWISE_180",
            Rotation::Counterclockwise90 => "COUNTERCLOCKWISE_90",
        }
    }

    pub fn rotate(self, direction: Direction) -> Direction {
        if direction.is_vertical() {
            return direction;
        }
        match self {
            Rotation::None => direction,
            Rotation::Clockwise90 => direction.clockwise(),
            Rotation::Clockwise180 => direction.opposite(),
            Rotation::Counterclockwise90 => direction.opposite().clockwise(),
        }
    }
}

/// The `CLOCKWISE_90` spelling the piece NBT uses.
pub mod legacy {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        rotation: &Rotation,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        legacy_serialize(rotation.legacy_name(), serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Rotation, D::Error> {
        legacy_deserialize(deserializer, &Rotation::ALL, Rotation::legacy_name)
    }
}

pub(crate) fn legacy_serialize<S: serde::Serializer>(
    name: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(name)
}

pub(crate) fn legacy_deserialize<'de, D: serde::Deserializer<'de>, T: Copy>(
    deserializer: D,
    all: &[T],
    name_of: fn(T) -> &'static str,
) -> Result<T, D::Error> {
    let name = String::deserialize(deserializer)?;
    all.iter()
        .copied()
        .find(|value| name_of(*value) == name)
        .ok_or_else(|| serde::de::Error::custom(format!("No value with id: {name}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_and_legacy_names_round_trip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Legacy(#[serde(with = "legacy")] Rotation);

        for rotation in Rotation::ALL {
            let json = serde_json::to_string(&rotation).unwrap();
            assert_eq!(serde_json::from_str::<Rotation>(&json).unwrap(), rotation);
            let legacy = serde_json::to_string(&Legacy(rotation)).unwrap();
            assert_eq!(
                serde_json::from_str::<Legacy>(&legacy).unwrap(),
                Legacy(rotation)
            );
        }
        assert_eq!(
            serde_json::to_string(&Rotation::Clockwise180).unwrap(),
            "\"180\""
        );
        assert_eq!(
            serde_json::to_string(&Legacy(Rotation::Clockwise180)).unwrap(),
            "\"CLOCKWISE_180\""
        );
        assert!(serde_json::from_str::<Legacy>("\"180\"").is_err());
    }
}
