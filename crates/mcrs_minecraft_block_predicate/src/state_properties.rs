use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_core::codec::unique_entries;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValueMatcher {
    Exact(String),
    Ranged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<String>,
    },
}

/// A property value as its position in the property's value order, and the
/// reader that places a written value in that order.
#[derive(Clone, Copy)]
struct Ranked {
    value: u8,
    rank: fn(&str) -> Option<u8>,
}

impl ValueMatcher {
    /// A written value or bound that is not a value of the property never
    /// matches.
    fn matches(&self, property: Ranked) -> bool {
        let bound = |text: &Option<String>, holds: fn(u8, u8) -> bool| {
            text.as_deref().is_none_or(|text| {
                (property.rank)(text).is_some_and(|bound| holds(bound, property.value))
            })
        };
        match self {
            ValueMatcher::Exact(text) => (property.rank)(text) == Some(property.value),
            ValueMatcher::Ranged { min, max } => {
                bound(min, |min, value| min <= value) && bound(max, |max, value| value <= max)
            }
        }
    }
}

/// Property name to matcher, in the order read.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StatePropertiesPredicate(pub Vec<(String, ValueMatcher)>);

impl StatePropertiesPredicate {
    pub const ANY: Self = StatePropertiesPredicate(Vec::new());

    pub fn is_any(&self) -> bool {
        self.0.is_empty()
    }

    /// Every entry has to name a property the state has, so any entry fails on
    /// the empty fluid.
    pub fn matches_fluid(&self, state: Option<&FluidState>) -> bool {
        self.0.iter().all(|(name, matcher)| {
            state
                .and_then(|state| fluid_property(state, name))
                .is_some_and(|property| matcher.matches(property))
        })
    }
}

fn fluid_property(state: &FluidState, name: &str) -> Option<Ranked> {
    match name {
        FluidState::LEVEL => Some(Ranked {
            value: state.level,
            rank: |text| {
                text.parse()
                    .ok()
                    .filter(|level| FluidState::LEVELS.contains(level))
            },
        }),
        FluidState::SOURCE => Some(Ranked {
            value: u8::from(state.source),
            rank: |text| text.parse::<bool>().ok().map(u8::from),
        }),
        _ => None,
    }
}

impl Serialize for StatePropertiesPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.iter().map(|(name, matcher)| (name, matcher)))
    }
}

impl<'de> Deserialize<'de> for StatePropertiesPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        unique_entries(d).map(StatePropertiesPredicate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_block::keys::Fluid;

    fn water(level: u8, source: bool) -> FluidState {
        FluidState {
            fluid: Fluid::Water.id(),
            level,
            source,
        }
    }

    #[test]
    fn a_fluid_state_matches_its_source_and_level_by_value_order() {
        let source = water(8, true);
        let sheet = water(7, false);
        let shallow = water(6, false);
        let waterfall = water(8, false);
        let cases: [(&str, &[(Option<&FluidState>, bool)]); 6] = [
            (
                r#"{"source": "true"}"#,
                &[(Some(&source), true), (Some(&sheet), false), (None, false)],
            ),
            (
                r#"{"level": {"min": "7"}}"#,
                &[
                    (Some(&source), true),
                    (Some(&sheet), true),
                    (Some(&shallow), false),
                ],
            ),
            (
                r#"{"source": "false", "level": "8"}"#,
                &[(Some(&waterfall), true), (Some(&source), false)],
            ),
            (
                r#"{"source": {"max": "false"}}"#,
                &[(Some(&sheet), true), (Some(&source), false)],
            ),
            (
                r#"{"falling": "false"}"#,
                &[(Some(&source), false), (Some(&sheet), false)],
            ),
            (
                r#"{"level": {"min": "9"}}"#,
                &[(Some(&source), false), (Some(&sheet), false)],
            ),
        ];
        for (json, states) in cases {
            let predicate: StatePropertiesPredicate = serde_json::from_str(json).unwrap();
            for (state, expected) in states.iter() {
                assert_eq!(
                    predicate.matches_fluid(*state),
                    *expected,
                    "{json} against {state:?}"
                );
            }
        }
        assert!(StatePropertiesPredicate::ANY.matches_fluid(None));
    }

    #[test]
    fn a_value_that_is_neither_a_string_nor_a_bound_fails_the_load() {
        assert!(serde_json::from_str::<StatePropertiesPredicate>(r#"{"source": true}"#).is_err());
    }
}
