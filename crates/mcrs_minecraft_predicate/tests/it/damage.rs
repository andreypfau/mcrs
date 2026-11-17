use glam::DVec3;
use mcrs_minecraft_predicate::{
    BakedDamageSourcePredicate, DamageEvent, DamagePredicate, DamageSource, DamageSourcePredicate,
    PredicateIssue, PredicateScratch,
};
use mcrs_minecraft_registries::Resolve;

use crate::crowd::{Crowd, Mob, tables};
use crate::grid::{Grid, definitions, id, registries};
use crate::item_predicate::TABLE;

const SKELETON: usize = 0;
const ARROW: usize = 1;
const PLAYER: usize = 2;

fn world() -> Crowd {
    let of_type = |name: &str, x: f64| Mob {
        entity_type: Some(id("minecraft:entity_type", name)),
        position: Some(DVec3::new(x, 64.0, 0.5)),
        ..Mob::default()
    };
    Crowd {
        grid: Grid::default(),
        mobs: vec![
            of_type("minecraft:skeleton", 0.5),
            of_type("minecraft:arrow", 0.5),
            of_type("minecraft:player", 10.5),
        ],
    }
}

fn source(damage_type: &str, direct: Option<usize>, causing: Option<usize>) -> DamageSource<usize> {
    DamageSource {
        damage_type: id("minecraft:damage_type", damage_type),
        direct_entity: direct,
        causing_entity: causing,
    }
}

fn shot() -> DamageSource<usize> {
    source("minecraft:arrow", Some(ARROW), Some(SKELETON))
}

fn fall() -> DamageSource<usize> {
    source("minecraft:fall", None, None)
}

fn hit() -> DamageSource<usize> {
    source("minecraft:mob_attack", Some(SKELETON), Some(SKELETON))
}

fn try_bake_source(json: &str) -> Result<BakedDamageSourcePredicate, PredicateIssue> {
    let named = serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"));
    DamageSourcePredicate::resolve(named, registries())
        .unwrap_or_else(|error| panic!("{json}: {error}"))
        .bake(registries(), definitions(), &*TABLE)
}

fn source_passes(json: &str, subject: Option<DamageSource<usize>>) -> bool {
    let mut scratch = PredicateScratch::default();
    try_bake_source(json).unwrap().matches(
        &world(),
        Some(DVec3::new(0.5, 64.0, 0.5)),
        subject,
        tables(),
        &mut scratch,
    )
}

#[test]
fn the_fields_of_a_damage_source_predicate_test_the_source_pred_dmg_001_to_003() {
    let projectile = r##"{"tags": [{"id": "#minecraft:is_projectile", "expected": true}]}"##;
    let not_projectile = r##"{"tags": [{"id": "#minecraft:is_projectile", "expected": false}]}"##;
    let cases: [(&str, &str, Option<DamageSource<usize>>, bool); 18] = [
        ("PRED-DMG-001", "{}", Some(fall()), true),
        ("PRED-DMG-001", "{}", None, false),
        ("PRED-DMG-002", projectile, Some(shot()), true),
        ("PRED-DMG-002", projectile, Some(fall()), false),
        ("PRED-DMG-002", not_projectile, Some(fall()), true),
        ("PRED-DMG-002", not_projectile, Some(shot()), false),
        (
            "PRED-DMG-002",
            r#"{"direct_entity": {"entity_type": "minecraft:arrow"}}"#,
            Some(shot()),
            true,
        ),
        (
            "PRED-DMG-002",
            r#"{"direct_entity": {"entity_type": "minecraft:skeleton"}}"#,
            Some(shot()),
            false,
        ),
        (
            "PRED-DMG-002",
            r#"{"direct_entity": {}}"#,
            Some(fall()),
            false,
        ),
        (
            "PRED-DMG-002",
            r#"{"source_entity": {"entity_type": "minecraft:skeleton"}}"#,
            Some(shot()),
            true,
        ),
        (
            "PRED-DMG-002",
            r#"{"source_entity": {"entity_type": "minecraft:arrow"}}"#,
            Some(shot()),
            false,
        ),
        (
            "PRED-DMG-002",
            r#"{"source_entity": {}}"#,
            Some(fall()),
            false,
        ),
        (
            "PRED-DMG-002",
            r#"{"source_entity": {"distance": {"absolute": {"max": 1.0}}}}"#,
            Some(shot()),
            true,
        ),
        (
            "PRED-DMG-003",
            r#"{"is_direct": true}"#,
            Some(shot()),
            false,
        ),
        (
            "PRED-DMG-003",
            r#"{"is_direct": false}"#,
            Some(shot()),
            true,
        ),
        ("PRED-DMG-003", r#"{"is_direct": true}"#, Some(fall()), true),
        ("PRED-DMG-003", r#"{"is_direct": true}"#, Some(hit()), true),
        (
            "PRED-DMG-003",
            r#"{"is_direct": true}"#,
            Some(source("minecraft:arrow", Some(ARROW), None)),
            false,
        ),
    ];
    for (rule, json, subject, expected) in cases {
        assert_eq!(source_passes(json, subject), expected, "{rule}: {json}");
    }
}

#[test]
fn damage_source_entity_predicate_has_nesting_bound_pred_ent_023() {
    let mut nested = "{}".to_owned();
    for _ in 1..33 {
        nested = format!(r#"{{"vehicle": {nested}}}"#);
    }
    let json = format!(r#"{{"direct_entity": {nested}}}"#);
    assert_eq!(try_bake_source(&json), Err(PredicateIssue::NestingTooDeep));
}

#[test]
fn each_entity_predicate_of_a_source_has_its_own_budget_pred_ent_024() {
    let rider = Mob {
        vehicle: Some(0),
        ..Mob::default()
    };
    let mut mobs = vec![Mob {
        passengers: Some((1..=8).collect()),
        ..Mob::default()
    }];
    mobs.extend((1..8).map(|_| rider.clone()));
    mobs.push(Mob {
        attack_target: Some(0),
        ..rider
    });
    let world = Crowd {
        grid: Grid::default(),
        mobs,
    };
    let mut json = r#"{"targeted_entity": {}}"#.to_owned();
    for _ in 0..4 {
        json = format!(r#"{{"vehicle": {{"passenger": {json}}}, "targeted_entity": {{}}}}"#);
    }
    let json = format!(
        r#"{{"direct_entity": {{"passenger": {json}}}, "source_entity": {{"passenger": {json}}}}}"#
    );
    let mut scratch = PredicateScratch::default();
    let passes = try_bake_source(&json).unwrap().matches(
        &world,
        None,
        Some(source("minecraft:mob_attack", Some(0), Some(0))),
        tables(),
        &mut scratch,
    );
    assert!(passes, "each entity predicate stays within its budget");
}

#[test]
fn the_fields_of_a_damage_predicate_test_the_event_pred_dmg_004_005() {
    let event = DamageEvent {
        player: PLAYER,
        source: shot(),
        dealt: 5.0,
        taken: 3.0,
        blocked: false,
    };
    let cases: [(&str, bool); 14] = [
        ("{}", true),
        (r#"{"dealt": {"min": 5.0}}"#, true),
        (r#"{"dealt": {"min": 5.5}}"#, false),
        (r#"{"taken": {"max": 3.0}}"#, true),
        (r#"{"taken": {"max": 2.5}}"#, false),
        (r#"{"blocked": false}"#, true),
        (r#"{"blocked": true}"#, false),
        (
            r#"{"source_entity": {"entity_type": "minecraft:skeleton"}}"#,
            true,
        ),
        (
            r#"{"source_entity": {"entity_type": "minecraft:arrow"}}"#,
            false,
        ),
        (
            r#"{"source_entity": {"distance": {"absolute": {"min": 9.0, "max": 11.0}}}}"#,
            true,
        ),
        (
            r#"{"source_entity": {"distance": {"absolute": {"max": 1.0}}}}"#,
            false,
        ),
        (r#"{"type": {"is_direct": false}}"#, true),
        (r#"{"type": {"is_direct": true}}"#, false),
        (
            r#"{"type": {"direct_entity": {"distance": {"absolute": {"max": 1.0}}}}}"#,
            false,
        ),
    ];
    let mut scratch = PredicateScratch::default();
    for (json, expected) in cases {
        let named = serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"));
        let predicate = DamagePredicate::resolve(named, registries())
            .unwrap()
            .bake(registries(), definitions(), &*TABLE)
            .unwrap();
        let passes = predicate.matches(&world(), &event, tables(), &mut scratch);
        assert_eq!(passes, expected, "PRED-DMG-005: {json}");
    }
}
