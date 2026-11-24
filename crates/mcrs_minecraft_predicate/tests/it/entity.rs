use glam::DVec3;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, ItemStack, Kind};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_predicate::{
    ActiveEffect, BakedEntityPredicate, BlockHost, EntityPredicate, EntityView, LocationView,
    PredicateIssue, PredicateScratch,
};
use mcrs_minecraft_registries::{Named, Resolve};
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

use crate::crowd::{Crowd, Held, Mob, Worn, tables};
use crate::grid::{Grid, definitions, id, registries, state};
use crate::item_predicate::{TABLE, sword};

const ORIGIN: DVec3 = DVec3::new(3.5, 64.0, 0.5);

fn decode(json: &str) -> Result<EntityPredicate<Named>, serde_json::Error> {
    serde_json::from_str(json)
}

fn try_bake(json: &str) -> Result<BakedEntityPredicate, PredicateIssue> {
    let named = decode(json).unwrap_or_else(|error| panic!("{json}: {error}"));
    let predicate = EntityPredicate::resolve(named, registries()).unwrap();
    predicate.bake(registries(), definitions(), &*TABLE)
}

fn bake(json: &str) -> BakedEntityPredicate {
    try_bake(json).unwrap()
}

fn passes<W: LocationView + EntityView>(json: &str, world: &W, entity: Option<W::Entity>) -> bool {
    let mut scratch = PredicateScratch::default();
    bake(json).matches(world, Some(ORIGIN), entity, tables(), &mut scratch)
}

fn stick() -> ItemStack {
    let item: Id<Item> = id("minecraft:item", "minecraft:stick");
    ItemStack::new(Kind(item.number()), Count::new(1).unwrap())
}

fn zombie() -> Mob {
    Mob {
        entity_type: Some(id("minecraft:entity_type", "minecraft:zombie")),
        position: Some(DVec3::new(0.5, 64.0, 0.5)),
        living: Some(true),
        ..Mob::default()
    }
}

fn crowd(mobs: Vec<Mob>) -> Crowd {
    Crowd {
        grid: Grid {
            blocks: vec![
                (BlockPos::new(0, 64, 0), state("minecraft:air", &[])),
                (BlockPos::new(0, 63, 0), state("minecraft:stone", &[])),
            ],
            ..Grid::default()
        },
        mobs,
    }
}

fn one(mob: Mob) -> Crowd {
    crowd(vec![mob])
}

#[test]
fn an_absent_entity_fails_each_predicate_pred_ent_001() {
    let world = one(zombie());
    assert!(passes("{}", &world, Some(0)));
    assert!(!passes("{}", &world, None));
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn each_sub_predicate_adds_its_test_pred_ent_005_to_022() {
    let speed = id("minecraft:mob_effect", "minecraft:speed");
    let effect = ActiveEffect {
        amplifier: 1,
        duration: -1,
        ambient: false,
        visible: true,
    };
    let mut saved = NbtCompound::new();
    saved.put_string("CustomName", "Bob".to_owned());
    let skeleton = Mob {
        entity_type: Some(id("minecraft:entity_type", "minecraft:skeleton")),
        ..zombie()
    };
    let grounded = Mob {
        on_ground: Some(true),
        support: Some(BlockPos::new(0, 63, 0)),
        ..zombie()
    };
    let moving = Mob {
        movement: Some(DVec3::new(0.25, -0.5, 0.0)),
        fall_distance: Some(3.5),
        ..zombie()
    };
    let item = Mob {
        entity_type: Some(id("minecraft:entity_type", "minecraft:item")),
        living: Some(false),
        ..zombie()
    };
    let tagged = Mob {
        tags: Some(vec!["red".to_owned(), "blue".to_owned()]),
        team: Some("guards".to_owned()),
        ..zombie()
    };

    let cases: Vec<(&str, &str, Mob, bool)> = vec![
        (
            "PRED-ENT-005",
            r#"{"entity_type": "minecraft:zombie"}"#,
            zombie(),
            true,
        ),
        (
            "PRED-ENT-005",
            r#"{"minecraft:entity_type": "minecraft:skeleton"}"#,
            zombie(),
            false,
        ),
        (
            "PRED-ENT-005",
            r##"{"entity_type": "#minecraft:skeletons"}"##,
            skeleton,
            true,
        ),
        (
            "PRED-ENT-006",
            r#"{"location": {"block": {"blocks": "minecraft:air"}}}"#,
            zombie(),
            true,
        ),
        (
            "PRED-ENT-006",
            r#"{"location": {"position": {"y": {"min": 64.5}}}}"#,
            zombie(),
            false,
        ),
        (
            "PRED-ENT-007",
            r#"{"stepping_on": {"position": {"y": 63.5}, "block": {"blocks": "minecraft:stone"}}}"#,
            grounded.clone(),
            true,
        ),
        (
            "PRED-ENT-007",
            r#"{"stepping_on": {"position": {"y": 64.0}}}"#,
            grounded.clone(),
            false,
        ),
        (
            "PRED-ENT-007",
            r#"{"stepping_on": {}}"#,
            Mob {
                on_ground: Some(false),
                ..grounded.clone()
            },
            false,
        ),
        (
            "PRED-ENT-008",
            r#"{"movement_affected_by": {"position": {"y": 63.5}}}"#,
            Mob {
                on_ground: Some(false),
                movement_position: Some(BlockPos::new(0, 63, 0)),
                ..zombie()
            },
            true,
        ),
        (
            "PRED-ENT-009",
            r#"{"distance": {"absolute": {"max": 3.0}}}"#,
            zombie(),
            true,
        ),
        (
            "PRED-ENT-009",
            r#"{"distance": {"absolute": {"max": 2.99}}}"#,
            zombie(),
            false,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"x": 5.0, "y": {"max": -10.0}}}"#,
            moving.clone(),
            true,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"x": {"min": 5.01}}}"#,
            moving.clone(),
            false,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"vertical_speed": 10.0}}"#,
            moving.clone(),
            true,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"horizontal_speed": {"max": 5.0}}}"#,
            moving.clone(),
            true,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"horizontal_speed": {"max": 4.99}}}"#,
            moving.clone(),
            false,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"speed": {"min": 11.18}}}"#,
            moving.clone(),
            true,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"speed": {"min": 11.19}}}"#,
            moving.clone(),
            false,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"fall_distance": {"min": 3.5}}}"#,
            moving.clone(),
            true,
        ),
        (
            "PRED-ENT-010",
            r#"{"movement": {"fall_distance": {"min": 3.6}}}"#,
            moving,
            false,
        ),
        (
            "PRED-EFX-001",
            r#"{"effects": {"minecraft:speed": {"amplifier": 1, "duration": -1, "visible": true}}}"#,
            Mob {
                effects: vec![(speed, effect)],
                ..zombie()
            },
            true,
        ),
        (
            "PRED-EFX-001",
            r#"{"effects": {"minecraft:speed": {"amplifier": {"min": 2}}}}"#,
            Mob {
                effects: vec![(speed, effect)],
                ..zombie()
            },
            false,
        ),
        (
            "PRED-EFX-001",
            r#"{"effects": {"minecraft:speed": {"ambient": true}}}"#,
            Mob {
                effects: vec![(speed, effect)],
                ..zombie()
            },
            false,
        ),
        (
            "PRED-EFX-001",
            r#"{"effects": {"minecraft:speed": {}}}"#,
            zombie(),
            false,
        ),
        ("PRED-EFX-001", r#"{"effects": {}}"#, zombie(), true),
        ("PRED-EFX-001", r#"{"effects": {}}"#, item.clone(), false),
        (
            "PRED-ENT-013",
            r#"{"nbt": "{CustomName:\"Bob\"}"}"#,
            Mob {
                saved_data: Some(saved.clone()),
                ..zombie()
            },
            true,
        ),
        (
            "PRED-ENT-013",
            r#"{"nbt": "{CustomName:\"Ann\"}"}"#,
            Mob {
                saved_data: Some(saved),
                ..zombie()
            },
            false,
        ),
        (
            "PRED-ENT-014",
            r#"{"flags": {"is_sneaking": true, "is_on_fire": false}}"#,
            Mob {
                pose: Some(Pose::Crouching),
                burning: Some(false),
                ..zombie()
            },
            true,
        ),
        (
            "PRED-ENT-014",
            r#"{"flags": {"is_sneaking": true}}"#,
            Mob {
                pose: Some(Pose::Swimming),
                ..zombie()
            },
            false,
        ),
        (
            "PRED-ENT-014",
            r#"{"flags": {"is_flying": true}}"#,
            Mob {
                fall_flying: Some(false),
                flying_ability: Some(true),
                ..zombie()
            },
            true,
        ),
        (
            "PRED-ENT-014",
            r#"{"flags": {"is_flying": true}}"#,
            item.clone(),
            false,
        ),
        (
            "PRED-ENT-014",
            r#"{"flags": {"is_flying": false}}"#,
            item.clone(),
            true,
        ),
        (
            "PRED-ENT-014",
            r#"{"flags": {"is_baby": true, "is_fall_flying": true}}"#,
            item,
            true,
        ),
        (
            "PRED-ENT-014",
            r#"{"flags": {"is_baby": true}}"#,
            Mob {
                baby: Some(false),
                ..zombie()
            },
            false,
        ),
        (
            "PRED-ENT-016",
            r#"{"periodic_tick": 5}"#,
            Mob {
                tick_count: Some(10),
                ..zombie()
            },
            true,
        ),
        (
            "PRED-ENT-016",
            r#"{"periodic_tick": 3}"#,
            Mob {
                tick_count: Some(10),
                ..zombie()
            },
            false,
        ),
        (
            "PRED-ENT-018",
            r#"{"team": "guards"}"#,
            tagged.clone(),
            true,
        ),
        (
            "PRED-ENT-018",
            r#"{"team": "thieves"}"#,
            tagged.clone(),
            false,
        ),
        ("PRED-ENT-018", r#"{"team": "guards"}"#, zombie(), false),
        (
            "PRED-ENT-022",
            r#"{"entity_tags": {"any_of": ["green", "blue"]}}"#,
            tagged.clone(),
            true,
        ),
        (
            "PRED-ENT-022",
            r#"{"entity_tags": {"any_of": []}}"#,
            tagged.clone(),
            false,
        ),
        (
            "PRED-ENT-022",
            r#"{"entity_tags": {"all_of": ["red", "green"]}}"#,
            tagged.clone(),
            false,
        ),
        (
            "PRED-ENT-022",
            r#"{"entity_tags": {"all_of": [], "none_of": []}}"#,
            tagged.clone(),
            true,
        ),
        (
            "PRED-ENT-022",
            r#"{"entity_tags": {"none_of": ["red"]}}"#,
            tagged,
            false,
        ),
    ];

    for (rule, json, mob, expected) in cases {
        assert_eq!(passes(json, &one(mob), Some(0)), expected, "{rule}: {json}");
    }
}

#[test]
fn distance_fails_without_an_origin_pred_ent_009() {
    let mut scratch = PredicateScratch::default();
    let distance = bake(r#"{"distance": {}}"#);
    assert!(!distance.matches(&one(zombie()), None, Some(0), tables(), &mut scratch));
}

#[test]
fn nested_predicates_test_the_second_entity_pred_ent_017() {
    let pig = || Mob {
        entity_type: Some(id("minecraft:entity_type", "minecraft:pig")),
        ..zombie()
    };
    let world = crowd(vec![
        Mob {
            vehicle: Some(1),
            ..zombie()
        },
        Mob {
            passengers: Some(vec![0, 2]),
            ..pig()
        },
        Mob {
            passengers: Some(vec![3]),
            attack_target: Some(0),
            ..pig()
        },
        Mob {
            team: Some("deep".to_owned()),
            ..zombie()
        },
    ]);
    let cases: [(&str, usize, bool); 9] = [
        (r#"{"vehicle": {"entity_type": "minecraft:pig"}}"#, 0, true),
        (r#"{"vehicle": {"entity_type": "minecraft:cow"}}"#, 0, false),
        (r#"{"vehicle": {}}"#, 1, false),
        (
            r#"{"vehicle": {"passenger": {"entity_type": "minecraft:pig"}}}"#,
            0,
            true,
        ),
        (
            r#"{"passenger": {"entity_type": "minecraft:zombie"}}"#,
            1,
            true,
        ),
        (r#"{"passenger": {"team": "deep"}}"#, 1, false),
        (r#"{"passenger": {}}"#, 0, false),
        (r#"{"targeted_entity": {"vehicle": {}}}"#, 2, true),
        (r#"{"targeted_entity": {}}"#, 1, false),
    ];
    for (json, entity, expected) in cases {
        assert_eq!(
            passes(json, &world, Some(entity)),
            expected,
            "{json} for {entity}"
        );
    }
}

#[test]
fn a_test_without_its_host_value_fails_pred_gen_004() {
    let passing = ["{}", r#"{"movement": {}}"#];
    let failing = [
        r#"{"entity_type": "minecraft:zombie"}"#,
        r#"{"location": {}}"#,
        r#"{"stepping_on": {}}"#,
        r#"{"movement_affected_by": {}}"#,
        r#"{"distance": {}}"#,
        r#"{"movement": {"x": {}, "fall_distance": {"max": 100.0}}}"#,
        r#"{"effects": {}}"#,
        r#"{"nbt": "{}"}"#,
        r#"{"flags": {"is_on_ground": false}}"#,
        r#"{"flags": {"is_baby": true}}"#,
        r#"{"periodic_tick": 1}"#,
        r#"{"vehicle": {}}"#,
        r#"{"passenger": {}}"#,
        r#"{"targeted_entity": {}}"#,
        r#"{"team": ""}"#,
        r#"{"entity_tags": {"none_of": ["red"]}}"#,
        r#"{"equipment": {"mainhand": {}}}"#,
        r#"{"slots": {"contents": {}}}"#,
    ];
    for json in passing {
        assert!(passes(json, &BlockHost::none(), Some(())), "{json}");
    }
    for json in failing {
        assert!(!passes(json, &BlockHost::none(), Some(())), "{json}");
    }
}

#[test]
fn the_bake_refuses_a_nesting_deeper_than_its_bound() {
    let nest = |depth: usize| {
        let mut json = "{}".to_owned();
        for _ in 1..depth {
            json = format!(r#"{{"vehicle": {json}}}"#);
        }
        json
    };
    assert!(try_bake(&nest(32)).is_ok());
    assert_eq!(try_bake(&nest(33)), Err(PredicateIssue::NestingTooDeep));
    let struck = format!(
        r#"{{"type_specific/lightning": {{"entity_struck": {}}}}}"#,
        nest(32)
    );
    assert_eq!(try_bake(&struck), Err(PredicateIssue::NestingTooDeep));
    let looking = format!(
        r#"{{"type_specific/player": {{"looking_at": {}}}}}"#,
        nest(31)
    );
    assert!(try_bake(&looking).is_ok());
}

#[test]
fn the_decode_refuses_an_unknown_type_and_ignores_an_unknown_field_reg_fld_624() {
    let refused = [
        r#"{"minecraft:type_specific/creeper": {}}"#,
        r#"{"colour": "red"}"#,
        r#"{"team": "a", "minecraft:team": "b"}"#,
        r#"{"periodic_tick": 0}"#,
        r#"{"slots": {"hotbar.9": {}}}"#,
        r#"{"slots": {"hotbar.08": {}}}"#,
    ];
    let ignored = [
        r#"{"flags": {"is_glowing": true}}"#,
        r#"{"type_specific/sheep": {"sheared": true, "colour": "red"}}"#,
        r#"{"type_specific/player": {"input": {"crouch": true}}}"#,
        r#"{"equipment": {"hands": {}}}"#,
        r#"{"entity_tags": {"some_of": ["red"]}}"#,
    ];
    for json in refused {
        assert!(decode(json).is_err(), "{json}");
    }
    for json in ignored {
        assert!(decode(json).is_ok(), "{json}");
    }
}

#[test]
fn a_null_fails_only_at_the_top_level_reg_fld_011() {
    let cases = [
        (r#"{"location": null}"#, false),
        (r#"{"periodic_tick": null}"#, false),
        (r#"{"type_specific/sheep": null}"#, false),
        (r#"{"vehicle": {"team": null}}"#, false),
        (r#"{"location": {"block": null}}"#, true),
    ];
    for (json, loads) in cases {
        assert_eq!(decode(json).is_ok(), loads, "{json}");
    }
}

#[test]
fn an_empty_form_tests_only_what_its_rule_names_pred_gen_005() {
    let of_type = |name: &str| Mob {
        entity_type: Some(id("minecraft:entity_type", name)),
        ..zombie()
    };
    let item = Mob {
        living: Some(false),
        ..of_type("minecraft:item")
    };
    let cases: [(&str, Mob, bool); 11] = [
        (r#"{"type_specific/fishing_hook": {}}"#, zombie(), true),
        (r#"{"slots": {}}"#, item.clone(), true),
        (r#"{"equipment": {}}"#, zombie(), true),
        (r#"{"equipment": {}}"#, item, false),
        (
            r#"{"type_specific/sheep": {}}"#,
            of_type("minecraft:sheep"),
            true,
        ),
        (r#"{"type_specific/sheep": {}}"#, zombie(), false),
        (
            r#"{"type_specific/cube_mob": {"size": {}}}"#,
            of_type("minecraft:magma_cube"),
            true,
        ),
        (
            r#"{"type_specific/lightning": {}}"#,
            of_type("minecraft:lightning_bolt"),
            true,
        ),
        (
            r#"{"type_specific/player": {"food": {}}}"#,
            of_type("minecraft:player"),
            true,
        ),
        (r#"{"type_specific/player": {}}"#, zombie(), false),
        (
            r#"{"entity_type": "minecraft:sheep", "type_specific/player": {}}"#,
            of_type("minecraft:sheep"),
            false,
        ),
    ];
    for (json, mob, expected) in cases {
        assert_eq!(passes(json, &one(mob), Some(0)), expected, "{json}");
    }
}

#[test]
fn a_match_fails_past_its_bound_of_nested_entities() {
    let rider = Mob {
        vehicle: Some(0),
        ..zombie()
    };
    let mut mobs = vec![Mob {
        passengers: Some((1..=8).collect()),
        ..zombie()
    }];
    mobs.extend((1..8).map(|_| rider.clone()));
    mobs.push(Mob {
        attack_target: Some(0),
        ..rider
    });
    let world = crowd(mobs);
    let alternate = |levels: usize| {
        let mut json = r#"{"targeted_entity": {}}"#.to_owned();
        for _ in 0..levels {
            json = format!(r#"{{"vehicle": {{"passenger": {json}}}, "targeted_entity": {{}}}}"#);
        }
        format!(r#"{{"passenger": {json}}}"#)
    };
    assert!(
        passes(&alternate(2), &world, Some(0)),
        "about a thousand nested matches"
    );
    assert!(
        !passes(&alternate(6), &world, Some(0)),
        "millions of nested matches"
    );
}

#[test]
fn the_fields_without_a_match_decode_and_fail_at_the_match() {
    let world = one(zombie());
    let cases = [
        r#"{"components": {"minecraft:max_stack_size": 3}}"#,
        r#"{"predicates": {"minecraft:damage": {"damage": 2}}}"#,
        r#"{"type_specific/lightning": {"blocks_set_on_fire": 0, "entity_struck": {"entity_type": "minecraft:pig"}}}"#,
        r#"{"type_specific/fishing_hook": {"in_open_water": true}}"#,
        r#"{"type_specific/cube_mob": {"size": {"min": 2}}}"#,
        r#"{"type_specific/raider": {"has_raid": true}}"#,
        r#"{"type_specific/sheep": {"sheared": false}}"#,
        r#"{"type_specific/player": {"level": {"min": 1}, "food": {"level": 20, "saturation": {"max": 5.0}}, "gamemode": ["survival"], "stats": [{"type": "minecraft:custom", "stat": "minecraft:jump", "value": {"min": 1}}], "recipes": {"minecraft:bread": true}, "advancements": {"minecraft:story/root": true, "minecraft:story/mine_stone": {"get_stone": false}}, "looking_at": {"entity_type": "minecraft:pig"}, "input": {"jump": true}}}"#,
    ];
    for json in cases {
        assert!(!passes(json, &world, Some(0)), "{json}");
    }
}

#[test]
fn equipment_tests_the_stack_of_each_slot_pred_ent_015() {
    let names = [
        ("head", EquipmentSlot::Head),
        ("chest", EquipmentSlot::Chest),
        ("legs", EquipmentSlot::Legs),
        ("feet", EquipmentSlot::Feet),
        ("body", EquipmentSlot::Body),
        ("saddle", EquipmentSlot::Saddle),
        ("mainhand", EquipmentSlot::MainHand),
        ("offhand", EquipmentSlot::OffHand),
    ];
    let wearing = |slot: EquipmentSlot, stack: Option<ItemStack>| Mob {
        equipment: Some(vec![Worn { slot, stack }]),
        ..zombie()
    };
    for (name, slot) in names {
        let stick_in = format!(r#"{{"equipment": {{"{name}": {{"items": "minecraft:stick"}}}}}}"#);
        let any_in = format!(r#"{{"equipment": {{"{name}": {{}}}}}}"#);
        let empty_in = format!(r#"{{"equipment": {{"{name}": {{"count": 0}}}}}}"#);
        let item = Mob {
            entity_type: Some(id("minecraft:entity_type", "minecraft:item")),
            living: Some(false),
            ..wearing(slot, Some(stick()))
        };
        let absent = Mob {
            equipment: None,
            ..zombie()
        };
        let cases: [(&str, Mob, bool); 7] = [
            (&stick_in, wearing(slot, Some(stick())), true),
            (&stick_in, wearing(slot, Some(sword())), false),
            (&empty_in, wearing(slot, None), true),
            (&stick_in, wearing(slot, None), false),
            (&any_in, wearing(slot, None), true),
            (&any_in, item, false),
            (&any_in, absent, false),
        ];
        for (json, mob, expected) in cases {
            assert_eq!(passes(json, &one(mob), Some(0)), expected, "{json}");
        }
    }
}

#[test]
fn slots_pass_when_one_slot_of_the_range_passes_pred_ent_019_020() {
    let held = |slot: u16, stack: Option<ItemStack>| Held { slot, stack };
    let player = Mob {
        slots: Some(vec![
            held(9, Some(sword())),
            held(10, Some(stick())),
            held(103, None),
            held(105, Some(stick())),
        ]),
        ..zombie()
    };
    let absent = Mob {
        slots: None,
        ..zombie()
    };
    let cases: [(&str, Mob, bool); 8] = [
        (
            r#"{"slots": {"inventory.*": {"items": "minecraft:stick"}}}"#,
            player.clone(),
            true,
        ),
        (
            r#"{"slots": {"inventory.0": {"items": "minecraft:stick"}}}"#,
            player.clone(),
            false,
        ),
        (
            r#"{"slots": {"inventory.1": {"items": "minecraft:stick"}}}"#,
            player.clone(),
            true,
        ),
        (
            r#"{"slots": {"armor.*": {"items": "minecraft:stick"}}}"#,
            player.clone(),
            true,
        ),
        (
            r#"{"slots": {"armor.head": {"count": 0}}}"#,
            player.clone(),
            true,
        ),
        (
            r#"{"slots": {"inventory.1": {}, "hotbar.*": {"items": "minecraft:stick"}}}"#,
            player.clone(),
            false,
        ),
        (r#"{"slots": {"enderchest.*": {}}}"#, player, false),
        (r#"{"slots": {"inventory.*": {}}}"#, absent, false),
    ];
    for (json, mob, expected) in cases {
        assert_eq!(passes(json, &one(mob), Some(0)), expected, "{json}");
    }
}
