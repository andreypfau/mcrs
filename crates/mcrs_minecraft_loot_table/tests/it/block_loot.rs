use std::collections::BTreeMap;
use std::sync::LazyLock;

use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_engine_registries::{test_corpus, test_item_table};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_loot_table::{BlockLoot, LootCause, LootContext, LootIssue, LootSeed, Tool};
use mcrs_minecraft_resource::{Id, ResourceLocation};
use mcrs_voxel::{BlockPos, BlockStateId};

static LOOT: LazyLock<(BlockLoot, Vec<LootIssue>)> =
    LazyLock::new(|| BlockLoot::bake(blocks(), test_registries(), &test_item_table()));

const ORIGIN: BlockPos = BlockPos::new(0, 64, 0);
const EMPTY_HAND: Tool<'static> = Tool {
    item: None,
    enchantments: &[],
};

fn blocks() -> &'static BlockDefinitions {
    &test_corpus().0.0
}

fn state(block: &str, values: &[(&str, &str)]) -> BlockStateId {
    let entry = blocks().block(block).expect("a vanilla block");
    values
        .iter()
        .fold(entry.default_state_id, |state, (name, value)| {
            entry
                .with_text(state, name, value)
                .expect("a property value")
        })
}

fn item(name: &str) -> Id<Item> {
    test_registries()
        .entries::<Item>()
        .and_then(|items| items.by_name(ResourceLocation::read(name).ok()?))
        .expect("a vanilla item")
}

fn enchantment(name: &str) -> Id<Enchantment> {
    test_registries()
        .entries::<Enchantment>()
        .and_then(|enchantments| enchantments.by_name(ResourceLocation::read(name).ok()?))
        .expect("a vanilla enchantment")
}

/// A break of `state` at the origin, with the world `around` it.
struct Break<'a> {
    state: BlockStateId,
    tool: Tool<'a>,
    this_entity: bool,
    explosion_radius: Option<f32>,
    around: &'a [(BlockPos, BlockStateId)],
}

impl<'a> Break<'a> {
    fn of(state: BlockStateId) -> Self {
        Self {
            state,
            tool: EMPTY_HAND,
            this_entity: true,
            explosion_radius: None,
            around: &[],
        }
    }

    /// The drop list of the evaluation at `tick`, as item names and counts.
    fn drops(&self, tick: u64) -> Vec<(String, u8)> {
        let Some(table) = blocks().state(self.state).loot else {
            return Vec::new();
        };
        let cx = LootContext {
            origin: ORIGIN,
            state: self.state,
            tool: self.tool,
            this_entity: self.this_entity,
            explosion_radius: self.explosion_radius,
            block_at: |pos: BlockPos| {
                self.around
                    .iter()
                    .find(|(at, _)| *at == pos)
                    .map(|&(_, state)| state)
            },
        };
        let seed = LootSeed {
            world: 7,
            tick,
            cause: LootCause::Break,
        };
        let mut out = Vec::new();
        LOOT.0.drops(table, &cx, seed, &mut out);
        let items = test_registries().entries::<Item>().expect("items");
        out.iter()
            .map(|stack| {
                let name = items.name(stack.item).expect("a named item");
                (name.as_str().to_owned(), stack.count)
            })
            .collect()
    }

    /// How many of `n` evaluations gave each drop list.
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

/// LOOT-SRC-001: every block table of the vanilla data pack names only items, blocks,
/// enchantments and predicates that the load holds.
#[test]
fn loot_src_001_every_vanilla_block_table_bakes() {
    assert_eq!(LOOT.1, []);
}

/// LOOT-SRC-003: a block table without pools drops nothing.
#[test]
fn loot_src_003_a_table_without_pools_drops_nothing() {
    assert_eq!(Break::of(state("minecraft:spawner", &[])).drops(0), []);
}

/// LOOT-EVL-005, LOOT-CND-001, LOOT-SRC-005: stone drops itself to Silk Touch and cobblestone
/// to an empty hand.
#[test]
fn loot_evl_005_stone_drops_stone_to_silk_touch_and_cobblestone_otherwise() {
    let stone = state("minecraft:stone", &[]);
    assert_eq!(Break::of(stone).drops(0), one("minecraft:cobblestone", 1));
    let silk_touch = [(enchantment("minecraft:silk_touch"), 1)];
    let tool = Tool {
        item: Some(item("minecraft:wooden_pickaxe")),
        enchantments: &silk_touch,
    };
    assert_eq!(
        Break {
            tool,
            ..Break::of(stone)
        }
        .drops(0),
        one("minecraft:stone", 1)
    );
}

/// LOOT-MOD-004: `ore_drops` with Fortune III gives the factor 1 with the chance 2/5 and each
/// of 2 to 4 with the chance 1/5. Result L1.
#[test]
fn loot_mod_004_coal_ore_with_fortune_3_drops_1_to_4_coal() {
    let fortune = [(enchantment("minecraft:fortune"), 3)];
    let tool = Tool {
        item: Some(item("minecraft:diamond_pickaxe")),
        enchantments: &fortune,
    };
    let spread = Break {
        tool,
        ..Break::of(state("minecraft:coal_ore", &[]))
    }
    .spread(4000);
    assert_eq!(spread.len(), 4, "{spread:?}");
    assert_share(&spread, &one("minecraft:coal", 1), 4000, 0.4);
    for count in 2..=4 {
        assert_share(&spread, &one("minecraft:coal", count), 4000, 0.2);
    }
}

/// LOOT-CND-004: gravel drops flint with the chance 0.1 to an empty hand.
#[test]
fn loot_cnd_004_gravel_drops_flint_one_time_in_ten() {
    let spread = Break::of(state("minecraft:gravel", &[])).spread(4000);
    assert_eq!(spread.len(), 2, "{spread:?}");
    assert_share(&spread, &one("minecraft:flint", 1), 4000, 0.1);
}

/// LOOT-MOD-001, LOOT-MOD-003, LOOT-MOD-008: glowstone drops 2 to 4 dust; a count below 1 of
/// a brown mushroom block makes no stack. Result L3.
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

/// LOOT-SRC-005, LOOT-MOD-004: ripe wheat drops one wheat and 1 + B(3, 0.5714) seeds to an
/// empty hand; young wheat drops one seed. Result L3.
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

/// LOOT-CND-006, LOOT-SRC-006: a door drops from its lower part only.
#[test]
fn loot_cnd_006_a_door_drops_from_its_lower_part() {
    let lower = state("minecraft:oak_door", &[("half", "lower")]);
    let upper = state("minecraft:oak_door", &[("half", "upper")]);
    assert_eq!(Break::of(lower).drops(0), one("minecraft:oak_door", 1));
    assert_eq!(Break::of(upper).drops(0), []);
}

/// LOOT-CND-008, LOOT-SRC-006: tall grass drops from a part whose other part still stands.
#[test]
fn loot_cnd_008_tall_grass_drops_while_its_other_part_stands() {
    let shears = Tool {
        item: Some(item("minecraft:shears")),
        enchantments: &[],
    };
    let upper = state("minecraft:tall_grass", &[("half", "upper")]);
    let lower = state("minecraft:tall_grass", &[("half", "lower")]);
    let below = [(BlockPos::new(0, 63, 0), lower)];
    let standing = Break {
        tool: shears,
        around: &below,
        ..Break::of(upper)
    };
    assert_eq!(standing.drops(0), one("minecraft:short_grass", 2));
    let alone = Break {
        tool: shears,
        ..Break::of(upper)
    };
    assert_eq!(alone.drops(0), []);
}

/// LOOT-CND-009: snow drops only for a context with an entity. Result L5.
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

/// LOOT-CND-007: an amethyst cluster drops four shards to an item of its tag and two to an
/// empty hand.
#[test]
fn loot_cnd_007_match_tool_tests_the_item_of_the_tool() {
    let cluster = state("minecraft:amethyst_cluster", &[]);
    let pickaxe = Tool {
        item: Some(item("minecraft:iron_pickaxe")),
        enchantments: &[],
    };
    assert_eq!(
        Break {
            tool: pickaxe,
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

/// LOOT-CND-003, LOOT-MOD-002: an explosion of radius 1 keeps each item; a large one keeps
/// almost none.
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

/// LOOT-RND-001: the drops are a function of the key of the evaluation.
#[test]
fn loot_rnd_001_the_same_key_gives_the_same_drops() {
    let gravel = Break::of(state("minecraft:gravel", &[]));
    let first: Vec<_> = (0..200).map(|tick| gravel.drops(tick)).collect();
    let again: Vec<_> = (0..200).map(|tick| gravel.drops(tick)).collect();
    assert_eq!(first, again);
}
