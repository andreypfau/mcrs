use std::fmt;

use mcrs_minecraft_core::codec::{NonNegativeInt, int_value, is_default};
use mcrs_minecraft_item::{Holder, Text};
use mcrs_minecraft_particle::ParticleOptions;
use mcrs_minecraft_sound::SoundEvent;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoonPhase {
    FullMoon,
    WaningGibbous,
    ThirdQuarter,
    WaningCrescent,
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
}

impl MoonPhase {
    pub const ALL: [MoonPhase; 8] = [
        MoonPhase::FullMoon,
        MoonPhase::WaningGibbous,
        MoonPhase::ThirdQuarter,
        MoonPhase::WaningCrescent,
        MoonPhase::NewMoon,
        MoonPhase::WaxingCrescent,
        MoonPhase::FirstQuarter,
        MoonPhase::WaxingGibbous,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// The serialized name, which is also the name of the phase's texture.
    pub const fn name(self) -> &'static str {
        match self {
            MoonPhase::FullMoon => "full_moon",
            MoonPhase::WaningGibbous => "waning_gibbous",
            MoonPhase::ThirdQuarter => "third_quarter",
            MoonPhase::WaningCrescent => "waning_crescent",
            MoonPhase::NewMoon => "new_moon",
            MoonPhase::WaxingCrescent => "waxing_crescent",
            MoonPhase::FirstQuarter => "first_quarter",
            MoonPhase::WaxingGibbous => "waxing_gibbous",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriState {
    True,
    False,
    Default,
}

impl Serialize for TriState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TriState::True => serializer.serialize_bool(true),
            TriState::False => serializer.serialize_bool(false),
            TriState::Default => serializer.serialize_str("default"),
        }
    }
}

impl<'de> Deserialize<'de> for TriState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TriStateVisitor;

        impl Visitor<'_> for TriStateVisitor {
            type Value = TriState;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a boolean or `default`")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<TriState, E> {
                Ok(if v { TriState::True } else { TriState::False })
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<TriState, E> {
                match v {
                    "true" => Ok(TriState::True),
                    "false" => Ok(TriState::False),
                    "default" => Ok(TriState::Default),
                    _ => Err(E::unknown_variant(v, &["true", "false", "default"])),
                }
            }
        }

        deserializer.deserialize_any(TriStateVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BedRuleCondition {
    Always,
    WhenDark,
    Never,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BedRule {
    pub can_sleep: BedRuleCondition,
    pub can_set_spawn: BedRuleCondition,
    #[serde(default, skip_serializing_if = "is_default")]
    pub destroy_on_use: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub destroy_on_leave: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<Text>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmbientParticle {
    pub particle: ParticleOptions,
    #[serde(deserialize_with = "crate::attribute::spec::unit_f32")]
    pub probability: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Music {
    pub sound: Holder<SoundEvent>,
    pub min_delay: NonNegativeInt,
    pub max_delay: NonNegativeInt,
    #[serde(default, skip_serializing_if = "is_default")]
    pub replace_current_music: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundMusic {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Music>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creative: Option<Music>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underwater: Option<Music>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmbientMood {
    pub sound: Holder<SoundEvent>,
    #[serde(deserialize_with = "int_value")]
    pub tick_delay: i32,
    #[serde(deserialize_with = "int_value")]
    pub block_search_extent: i32,
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmbientAdditions {
    pub sound: Holder<SoundEvent>,
    pub tick_chance: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmbientSounds {
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_sound: Option<Holder<SoundEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mood: Option<AmbientMood>,
    #[serde(
        default,
        with = "mcrs_minecraft_core::codec::compact_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additions: Vec<AmbientAdditions>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moon_phase_is_named_and_numbered_as_it_is_written() {
        for (index, phase) in MoonPhase::ALL.into_iter().enumerate() {
            assert_eq!(phase.index(), index);
            assert_eq!(serde_json::to_value(phase).unwrap(), phase.name());
        }
    }

    #[test]
    fn a_tri_state_is_a_boolean_or_default() {
        for (text, state) in [
            ("true", TriState::True),
            ("false", TriState::False),
            ("\"default\"", TriState::Default),
        ] {
            let read: TriState = serde_json::from_str(text).unwrap();
            assert_eq!(read, state);
            assert_eq!(serde_json::to_string(&state).unwrap(), text);
        }
        assert!(serde_json::from_str::<TriState>("\"maybe\"").is_err());
    }

    #[test]
    fn one_ambient_addition_is_written_bare_and_two_as_a_list() {
        mcrs_minecraft_worldgen_testing::corpus_set()
            .scope(one_ambient_addition_is_written_bare_and_two_as_a_list_in_scope);
    }

    fn one_ambient_addition_is_written_bare_and_two_as_a_list_in_scope() {
        let one = r#"{"additions":{"sound":"minecraft:ambient.cave","tick_chance":0.5}}"#;
        let two = r#"{"additions":[{"sound":"minecraft:ambient.cave","tick_chance":0.5},{"sound":"minecraft:ambient.cave","tick_chance":0.25}]}"#;
        let listed = r#"{"additions":[{"sound":"minecraft:ambient.cave","tick_chance":0.5}]}"#;
        for (text, count) in [(one, 1), (two, 2), (listed, 1)] {
            let read: AmbientSounds = serde_json::from_str(text).unwrap();
            assert_eq!(read.additions.len(), count);
            let expected = if count == 1 { one } else { two };
            assert_eq!(serde_json::to_string(&read).unwrap(), expected);
        }
        let none = AmbientSounds::default();
        assert_eq!(serde_json::to_string(&none).unwrap(), "{}");
    }

    #[test]
    fn an_ambient_probability_outside_the_unit_interval_is_refused() {
        let read = |probability: f32| {
            serde_json::from_str::<AmbientParticle>(&format!(
                r#"{{"particle":{{"type":"minecraft:ash"}},"probability":{probability}}}"#
            ))
        };
        assert!(read(1.0).is_ok());
        assert!(read(1.5).is_err());
        assert!(read(-0.1).is_err());
    }
}
