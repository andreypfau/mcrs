use std::collections::BTreeMap;

use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_game_registries::registry_load::{load_registries, test_additions, test_packs};
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_loot_condition::ConditionProblem;
use mcrs_minecraft_loot_table::{LootProblem, LootScratch, LootTables};
use mcrs_minecraft_predicate::BlockHost;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::fixture::{Around, LOOT, block_table, blocks, context, names, state, stream, tool};

struct Break<'a> {
    state: BlockStateId,
    tool: Option<ItemStack>,
    this_entity: bool,
    explosion_radius: Option<f32>,
    around: &'a [(BlockPos, BlockStateId)],
}

impl Break<'_> {
    fn of(state: BlockStateId) -> Self {
        Self {
            state,
            tool: None,
            this_entity: true,
            explosion_radius: None,
            around: &[],
        }
    }

    fn drops(&self, tick: u64) -> Vec<(String, u8)> {
        let Some(loot_id) = blocks().state(self.state).loot else {
            return Vec::new();
        };
        let table = LOOT.0.block_table(loot_id).expect("a loaded table");
        let mut cx = context(self.state, self.tool.as_ref());
        cx.this_entity = self.this_entity.then_some(());
        cx.explosion_radius = self.explosion_radius;
        let mut stacks = Vec::new();
        LOOT.0
            .roll(
                table,
                &cx,
                &Around {
                    blocks: self.around,
                    main_hand: None,
                },
                &mut stream(&LOOT.0, table, tick),
                &mut LootScratch::new(),
                &mut stacks,
            )
            .expect("a block context");
        names(stacks.into_iter().filter(Option::is_some))
    }

    fn spread(&self, n: u64) -> BTreeMap<Vec<(String, u8)>, u64> {
        let mut spread = BTreeMap::new();
        for tick in 0..n {
            *spread.entry(self.drops(tick)).or_default() += 1;
        }
        spread
    }
}

fn one(item: &str, count: u8) -> Vec<(String, u8)> {
    vec![(item.to_owned(), count)]
}

#[expect(clippy::cast_precision_loss, reason = "the counts are small")]
fn assert_share(
    spread: &BTreeMap<Vec<(String, u8)>, u64>,
    drops: &[(String, u8)],
    n: u64,
    share: f64,
) {
    let seen = spread.get(drops).copied().unwrap_or(0) as f64 / n as f64;
    assert!(
        (seen - share).abs() < 0.03,
        "{drops:?}: share {seen}, expected {share}; {spread:?}"
    );
}

#[test]
fn loot_src_001_every_vanilla_block_table_bakes() {
    assert_eq!(LOOT.1, []);
}

#[test]
fn loot_src_003_a_table_without_pools_drops_nothing() {
    assert_eq!(Break::of(state("minecraft:spawner", &[])).drops(0), []);
}

#[test]
fn loot_evl_005_stone_drops_stone_to_silk_touch_and_cobblestone_otherwise() {
    let stone = state("minecraft:stone", &[]);
    assert_eq!(Break::of(stone).drops(0), one("minecraft:cobblestone", 1));
    let tool = tool("minecraft:wooden_pickaxe", &[("minecraft:silk_touch", 1)]);
    assert_eq!(
        Break {
            tool: Some(tool),
            ..Break::of(stone)
        }
        .drops(0),
        one("minecraft:stone", 1)
    );
}

#[test]
fn loot_mod_004_coal_ore_with_fortune_3_drops_1_to_4_coal() {
    let tool = tool("minecraft:diamond_pickaxe", &[("minecraft:fortune", 3)]);
    let spread = Break {
        tool: Some(tool),
        ..Break::of(state("minecraft:coal_ore", &[]))
    }
    .spread(4000);
    assert_eq!(spread.len(), 4, "{spread:?}");
    assert_share(&spread, &one("minecraft:coal", 1), 4000, 0.4);
    for count in 2..=4 {
        assert_share(&spread, &one("minecraft:coal", count), 4000, 0.2);
    }
}

#[test]
fn loot_cnd_004_gravel_drops_flint_one_time_in_ten() {
    let spread = Break::of(state("minecraft:gravel", &[])).spread(4000);
    assert_eq!(spread.len(), 2, "{spread:?}");
    assert_share(&spread, &one("minecraft:flint", 1), 4000, 0.1);
}

#[test]
fn loot_mod_003_counts_are_limited_and_a_count_of_0_drops_nothing() {
    let glowstone = Break::of(state("minecraft:glowstone", &[])).spread(900);
    let counts: Vec<_> = glowstone.keys().cloned().collect();
    assert_eq!(
        counts,
        [2, 3, 4].map(|count| one("minecraft:glowstone_dust", count))
    );
    let mushroom = Break::of(state("minecraft:brown_mushroom_block", &[])).spread(4000);
    assert_eq!(mushroom.len(), 3, "{mushroom:?}");
    assert_share(&mushroom, &[], 4000, 7.0 / 9.0);
    assert_share(
        &mushroom,
        &one("minecraft:brown_mushroom", 2),
        4000,
        1.0 / 9.0,
    );
}

#[test]
fn loot_src_005_ripe_wheat_drops_wheat_and_1_to_4_seeds() {
    let spread = Break::of(state("minecraft:wheat", &[("age", "7")])).spread(4000);
    let p = 0.571_428_6_f64;
    for (seeds, ways) in [(1, 1.0), (2, 3.0), (3, 3.0), (4, 1.0)] {
        let drops = [
            ("minecraft:wheat".to_owned(), 1),
            ("minecraft:wheat_seeds".to_owned(), seeds),
        ];
        let share = ways * p.powi(i32::from(seeds) - 1) * (1.0 - p).powi(4 - i32::from(seeds));
        assert_share(&spread, &drops, 4000, share);
    }
    assert_eq!(
        Break::of(state("minecraft:wheat", &[("age", "0")])).drops(0),
        one("minecraft:wheat_seeds", 1)
    );
}

#[test]
fn loot_cnd_006_a_door_drops_from_its_lower_part() {
    let lower = state("minecraft:oak_door", &[("half", "lower")]);
    let upper = state("minecraft:oak_door", &[("half", "upper")]);
    assert_eq!(Break::of(lower).drops(0), one("minecraft:oak_door", 1));
    assert_eq!(Break::of(upper).drops(0), []);
}

#[test]
fn loot_cnd_008_tall_grass_drops_while_its_other_part_stands() {
    let shears = tool("minecraft:shears", &[]);
    let upper = state("minecraft:tall_grass", &[("half", "upper")]);
    let lower = state("minecraft:tall_grass", &[("half", "lower")]);
    let below = [(BlockPos::new(0, 63, 0), lower)];
    let standing = Break {
        tool: Some(shears.clone()),
        around: &below,
        ..Break::of(upper)
    };
    assert_eq!(standing.drops(0), one("minecraft:short_grass", 2));
    let alone = Break {
        tool: Some(shears),
        ..Break::of(upper)
    };
    assert_eq!(alone.drops(0), []);
}

#[test]
fn loot_cnd_009_snow_drops_only_to_an_entity() {
    let snow = state("minecraft:snow", &[("layers", "3")]);
    assert_eq!(Break::of(snow).drops(0), one("minecraft:snowball", 3));
    let removed = Break {
        this_entity: false,
        ..Break::of(snow)
    };
    assert_eq!(removed.drops(0), []);
}

#[test]
fn loot_cnd_007_match_tool_tests_the_item_of_the_tool() {
    let cluster = state("minecraft:amethyst_cluster", &[]);
    assert_eq!(
        Break {
            tool: Some(tool("minecraft:iron_pickaxe", &[])),
            ..Break::of(cluster)
        }
        .drops(0),
        one("minecraft:amethyst_shard", 4)
    );
    assert_eq!(
        Break::of(cluster).drops(0),
        one("minecraft:amethyst_shard", 2)
    );
}

#[test]
fn loot_cnd_003_an_explosion_keeps_items_with_the_chance_1_over_its_radius() {
    let stone = state("minecraft:stone", &[]);
    let small = Break {
        explosion_radius: Some(1.0),
        ..Break::of(stone)
    };
    assert_eq!(small.drops(0), one("minecraft:cobblestone", 1));
    let large = Break {
        explosion_radius: Some(1.0e9),
        ..Break::of(stone)
    };
    assert_eq!(large.spread(100).keys().collect::<Vec<_>>(), [&Vec::new()]);
}

#[test]
fn loot_rnd_001_the_same_key_gives_the_same_drops() {
    let gravel = Break::of(state("minecraft:gravel", &[]));
    let first: Vec<_> = (0..200).map(|tick| gravel.drops(tick)).collect();
    let again: Vec<_> = (0..200).map(|tick| gravel.drops(tick)).collect();
    assert_eq!(first, again);
}

#[test]
fn reg_fld_701_the_terms_of_any_of_and_all_of_may_be_a_predicate_tag() {
    let cases = [
        ("stone", "any_of", r##""#t:mixed""##, true),
        ("granite", "any_of", r#"["t:pass", "t:fail"]"#, true),
        ("diorite", "all_of", r##""#t:mixed""##, false),
        ("andesite", "all_of", r#"["t:pass", "t:fail"]"#, false),
        ("dirt", "all_of", r##""#t:passing""##, true),
        ("coarse_dirt", "any_of", r##""#t:missing""##, false),
        ("cobblestone", "any_of", r##""#t:loop""##, false),
    ];
    let mut additions = test_additions();
    let files = &mut additions.files;
    for (name, chance) in [("pass", 1.0), ("fail", 0.0)] {
        files.insert(
            "tags",
            format!("data/t/predicate/{name}.json"),
            format!(r#"{{"type": "minecraft:random_chance", "chance": {chance:.1}}}"#),
        );
    }
    for name in ["loop_a", "loop_b", "loop_c", "loop_d"] {
        files.insert(
            "tags",
            format!("data/t/predicate/{name}.json"),
            r##"{"type": "minecraft:any_of", "terms": "#t:loop"}"##,
        );
    }
    for (name, values) in [
        ("mixed", r#"["t:pass", "t:fail"]"#),
        ("passing", r#"["t:pass"]"#),
        (
            "loop",
            r#"["t:loop_a", "t:loop_b", "t:loop_c", "t:loop_d"]"#,
        ),
    ] {
        files.insert(
            "tags",
            format!("data/t/tags/predicate/{name}.json"),
            format!(r#"{{"values": {values}}}"#),
        );
    }
    for (block, kind, terms, _) in cases {
        files.insert(
            "tags",
            format!("data/minecraft/loot_table/blocks/{block}.json"),
            format!(
                r#"{{"pools": [{{"rolls": 1, "condition": {{"type": "minecraft:{kind}", "terms": {terms}}},
                "entries": [{{"type": "minecraft:item", "name": "minecraft:stick"}}]}}]}}"#
            ),
        );
    }
    additions.embedded_packs.push("tags".to_owned());
    let (registries, _) = load_registries(&test_packs(), additions).expect("the pack loads");
    let (loot, issues) = LootTables::bake(blocks(), &registries, &test_item_table());

    let (looped, issues): (Vec<_>, Vec<_>) = issues
        .into_iter()
        .map(|issue| (issue.name.to_string(), issue.problem))
        .partition(|(entry, _)| entry.starts_with("t:loop_"));
    assert!(!looped.is_empty());
    assert_eq!(
        issues,
        [(
            "minecraft:blocks/coarse_dirt".to_owned(),
            LootProblem::Condition(ConditionProblem::UnknownPredicate("#t:missing".to_owned()))
        )]
    );
    for (block, kind, terms, passes) in cases {
        let state = state(&format!("minecraft:{block}"), &[]);
        let table = block_table(&loot, state);
        let mut out = Vec::new();
        loot.roll(
            table,
            &context(state, None),
            &BlockHost::none(),
            &mut stream(&loot, table, 0),
            &mut LootScratch::new(),
            &mut out,
        )
        .expect("a block context");
        assert_eq!(!out.is_empty(), passes, "{kind} {terms}");
    }
}
