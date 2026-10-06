use mcrs_minecraft_game_rule::{GameRule, GameRuleCategory, GameRuleValueType};

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
