#[rustfmt::skip]
pub mod keys;

use mcrs_minecraft_core::StaticResourceLocation;

pub use keys::GameRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameRuleCategory {
    Player,
    Mobs,
    Spawning,
    Drops,
    Updates,
    Chat,
    Misc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRuleValueType {
    Bool { default: bool },
    Int { default: i32, min: i32, max: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRuleDefinition {
    pub category: GameRuleCategory,
    pub value: GameRuleValueType,
    pub required_features: &'static [StaticResourceLocation],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_reads_its_dumped_definition() {
        let cases = [
            (
                GameRule::AdvanceTime,
                GameRuleCategory::Updates,
                GameRuleValueType::Bool { default: true },
            ),
            (
                GameRule::MaxSnowAccumulationHeight,
                GameRuleCategory::Updates,
                GameRuleValueType::Int {
                    default: 1,
                    min: 0,
                    max: 8,
                },
            ),
            (
                GameRule::RandomTickSpeed,
                GameRuleCategory::Updates,
                GameRuleValueType::Int {
                    default: 3,
                    min: 0,
                    max: i32::MAX,
                },
            ),
        ];
        for (rule, category, value) in cases {
            let definition = rule.definition();
            assert_eq!(
                (definition.category, definition.value),
                (category, value),
                "{rule:?}"
            );
        }
        let gated: Vec<_> = GameRule::ALL
            .iter()
            .filter(|rule| !rule.definition().required_features.is_empty())
            .collect();
        assert_eq!(gated, [&GameRule::MaxMinecartSpeed]);
    }
}
