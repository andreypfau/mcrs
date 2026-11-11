use mcrs_minecraft_game_rule::{
    AnyGameRule, GameRule, GameRuleCategory, GameRulePreset, GameRuleValueType, GameRuleValues,
    McrsGameRule,
};
use mcrs_minecraft_resource::rl;

#[test]
fn test_preset_parsing_and_names() {
    assert_eq!(GameRulePreset::parse("java"), Some(GameRulePreset::Java));
    assert_eq!(GameRulePreset::parse("Java"), Some(GameRulePreset::Java));
    assert_eq!(GameRulePreset::parse("JAVA"), Some(GameRulePreset::Java));
    assert_eq!(GameRulePreset::parse("bedrock"), Some(GameRulePreset::Bedrock));
    assert_eq!(GameRulePreset::parse("Bedrock"), Some(GameRulePreset::Bedrock));
    assert_eq!(GameRulePreset::parse("unknown"), None);

    assert_eq!(GameRulePreset::Java.as_str(), "java");
    assert_eq!(GameRulePreset::Bedrock.as_str(), "bedrock");
}

#[test]
fn test_all_mcrs_rules_have_definitions_and_citations() {
    for &rule in McrsGameRule::ALL {
        let def = rule.definition();
        match def.value {
            GameRuleValueType::Bool { .. } => {}
            GameRuleValueType::Int { min, max, default } => {
                assert!(default >= min, "Default must be >= min for {rule:?}");
                assert!(default <= max, "Default must be <= max for {rule:?}");
            }
        }
        let citation = rule.parity_citation();
        assert!(!citation.is_empty(), "Citation must not be empty for {rule:?}");
        assert!(citation.starts_with("PARITY-"), "Citation must cite PARITY list entry for {rule:?}");
    }
}

#[test]
fn test_candidate_rules_differ_between_java_and_bedrock_presets() {
    // Redstone: QC, sticky piston 1-tick, piston pushing block entities
    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::QuasiConnectivity), 1);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::QuasiConnectivity), 0);

    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::StickyPiston1Tick), 1);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::StickyPiston1Tick), 0);

    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::PistonPushBlockEntities), 0);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::PistonPushBlockEntities), 1);

    // Combat: attack charge cooldown scaling
    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::AttackChargeDamageScaling), 1);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::AttackChargeDamageScaling), 0);

    // Hunger: saturation boost, eating in peaceful
    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::SaturationBoost), 1);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::SaturationBoost), 0);

    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::EatingInPeaceful), 0);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::EatingInPeaceful), 1);

    // Damage: void depth and creative immunity
    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::VoidDepth), -64);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::VoidDepth), -104);

    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::CreativeVoidDamage), 1);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::CreativeVoidDamage), 0);

    // Snow: gravity and melting
    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::SnowLayerGravity), 0);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::SnowLayerGravity), 1);

    assert_eq!(GameRulePreset::Java.value_of_mcrs(McrsGameRule::SnowLayerMelting), 0);
    assert_eq!(GameRulePreset::Bedrock.value_of_mcrs(McrsGameRule::SnowLayerMelting), 1);

    // Vanilla differences: random tick speed and snow accumulation height
    assert_eq!(GameRulePreset::Java.value_of_vanilla(GameRule::RandomTickSpeed), 3);
    assert_eq!(GameRulePreset::Bedrock.value_of_vanilla(GameRule::RandomTickSpeed), 1);

    assert_eq!(GameRulePreset::Java.value_of_vanilla(GameRule::MaxSnowAccumulationHeight), 1);
    assert_eq!(GameRulePreset::Bedrock.value_of_vanilla(GameRule::MaxSnowAccumulationHeight), 8);
}

#[test]
fn test_game_rule_values_preset_and_independent_mutation() {
    let mut values = GameRuleValues::from_preset(GameRulePreset::Java);

    // Starts Java
    assert!(values.is_on(AnyGameRule::from(McrsGameRule::QuasiConnectivity)));
    assert!(!values.is_on(AnyGameRule::from(McrsGameRule::SnowLayerGravity)));

    // Server operator can turn on snow gravity while keeping Java QC and immovable block entities
    values.set(AnyGameRule::from(McrsGameRule::SnowLayerGravity), 1);
    assert!(values.is_on(AnyGameRule::from(McrsGameRule::SnowLayerGravity)));
    assert!(values.is_on(AnyGameRule::from(McrsGameRule::QuasiConnectivity)));
    assert!(!values.is_on(AnyGameRule::from(McrsGameRule::PistonPushBlockEntities)));

    // Re-applying Bedrock preset resets values to Bedrock defaults
    values.apply_preset(GameRulePreset::Bedrock);
    assert!(!values.is_on(AnyGameRule::from(McrsGameRule::QuasiConnectivity)));
    assert!(values.is_on(AnyGameRule::from(McrsGameRule::PistonPushBlockEntities)));
    assert!(values.is_on(AnyGameRule::from(McrsGameRule::SnowLayerGravity)));
}

#[test]
fn test_any_game_rule_from_location() {
    let vanilla_loc = rl!("minecraft:random_tick_speed");
    let any_vanilla = AnyGameRule::from_location(&vanilla_loc).expect("must parse vanilla rule");
    assert_eq!(any_vanilla, AnyGameRule::Vanilla(GameRule::RandomTickSpeed));
    assert_eq!(any_vanilla.definition().category, GameRuleCategory::Updates);

    let mcrs_loc = rl!("mcrs:piston_push_block_entities");
    let any_mcrs = AnyGameRule::from_location(&mcrs_loc).expect("must parse mcrs rule");
    assert_eq!(any_mcrs, AnyGameRule::Mcrs(McrsGameRule::PistonPushBlockEntities));
    assert_eq!(any_mcrs.definition().category, GameRuleCategory::Updates);
}
