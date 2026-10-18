use std::fmt;

use mcrs_minecraft_block::definition;
use mcrs_minecraft_block::keys::Fluid;
use mcrs_minecraft_core::Id;
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A fluid state as a data pack writes it. `None` is the empty fluid, which
/// has no properties.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidState(pub Option<definition::FluidState>);

#[derive(Deserialize)]
struct Repr {
    id: Fluid,
    properties: Option<Properties>,
}

#[derive(Deserialize, Default)]
struct Properties {
    level: Option<String>,
    source: Option<String>,
}

#[derive(Serialize)]
struct Written {
    id: Fluid,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<WrittenProperties>,
}

#[derive(Serialize)]
struct WrittenProperties {
    level: String,
    source: &'static str,
}

impl FluidState {
    fn written(&self) -> Result<Written, Id<Fluid>> {
        let Some(state) = self.0.filter(|state| state.fluid != Fluid::Empty.id()) else {
            return Ok(Written {
                id: Fluid::Empty,
                properties: None,
            });
        };
        Ok(Written {
            id: Fluid::from_id(state.fluid).ok_or(state.fluid)?,
            properties: Some(WrittenProperties {
                level: state.level.to_string(),
                source: if state.source { "true" } else { "false" },
            }),
        })
    }
}

impl<'de> Deserialize<'de> for FluidState {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let repr = Repr::deserialize(d)?;
        if repr.id == Fluid::Empty {
            return Ok(FluidState(None));
        }
        let mut state = definition::FluidState::source_of(repr.id.id());
        let properties = repr.properties.unwrap_or_default();
        if let Some(level) = properties.level {
            state.level = level
                .parse()
                .ok()
                .filter(|level| definition::FluidState::LEVELS.contains(level))
                .ok_or_else(|| {
                    D::Error::custom(format_args!("fluid level `{level}` is not 1 to 8"))
                })?;
        }
        if let Some(source) = properties.source {
            state.source = source.parse().map_err(|_| {
                D::Error::custom(format_args!("fluid source `{source}` is not a boolean"))
            })?;
        }
        Ok(FluidState(Some(state)))
    }
}

impl Serialize for FluidState {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.written()
            .map_err(|id| S::Error::custom(format_args!("{id:?} is not a fluid")))?
            .serialize(s)
    }
}

/// The data pack form, for messages.
impl fmt::Display for FluidState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.written() {
            Ok(Written {
                id,
                properties: None,
            }) => write!(f, r#"{{"id":"{}"}}"#, id.as_static_str()),
            Ok(Written {
                id,
                properties: Some(WrittenProperties { level, source }),
            }) => write!(
                f,
                r#"{{"id":"{}","properties":{{"level":"{level}","source":"{source}"}}}}"#,
                id.as_static_str()
            ),
            Err(id) => write!(f, "{id:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fluid_state_reads_from_its_default_and_writes_both_properties() {
        let cases = [
            (
                r#"{"id":"minecraft:lava","properties":{"source":"false","level":"3"}}"#,
                Some((Fluid::Lava, 3, false)),
                r#"{"id":"minecraft:lava","properties":{"level":"3","source":"false"}}"#,
            ),
            (
                r#"{"id":"minecraft:water","properties":{"falling":"true"}}"#,
                Some((Fluid::Water, 8, true)),
                r#"{"id":"minecraft:water","properties":{"level":"8","source":"true"}}"#,
            ),
            (
                r#"{"id":"minecraft:water"}"#,
                Some((Fluid::Water, 8, true)),
                r#"{"id":"minecraft:water","properties":{"level":"8","source":"true"}}"#,
            ),
            (
                r#"{"id":"minecraft:empty","properties":{"level":"3"}}"#,
                None,
                r#"{"id":"minecraft:empty"}"#,
            ),
        ];
        for (json, expected, written) in cases {
            let state: FluidState = serde_json::from_str(json).unwrap();
            let expected = expected.map(|(fluid, level, source)| definition::FluidState {
                fluid: fluid.id(),
                level,
                source,
            });
            assert_eq!(state.0, expected, "{json}");
            assert_eq!(serde_json::to_string(&state).unwrap(), written, "{json}");
            assert_eq!(state.to_string(), written, "{json}");
            let read_back: FluidState = serde_json::from_str(written).unwrap();
            assert_eq!(read_back, state, "{written}");
        }
        let stated_empty = FluidState(Some(definition::FluidState::source_of(Fluid::Empty.id())));
        assert_eq!(
            serde_json::to_string(&stated_empty).unwrap(),
            r#"{"id":"minecraft:empty"}"#
        );
    }

    #[test]
    fn an_unknown_fluid_or_a_malformed_property_fails_the_load() {
        for json in [
            r#"{"id":"minecraft:flowing_water"}"#,
            r#"{"id":"minecraft:water","properties":{"level":"9"}}"#,
            r#"{"id":"minecraft:water","properties":{"level":8}}"#,
            r#"{"id":"minecraft:water","properties":{"source":"yes"}}"#,
        ] {
            assert!(serde_json::from_str::<FluidState>(json).is_err(), "{json}");
        }
    }
}
