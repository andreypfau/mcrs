use glam::DVec3;
use mcrs_minecraft_predicate::{LocationPredicate, PredicateScratch};
use mcrs_minecraft_registries::Resolve;
use mcrs_voxel::BlockPos;

use crate::grid::{Grid, definitions, registries, smoke, state};

fn passes(state_json: &str, block: &str, properties: &[(&str, &str)]) -> bool {
    let json = format!(r#"{{"block": {{"blocks": "{block}", "state": {state_json}}}}}"#);
    let named = serde_json::from_str(&json).unwrap();
    let predicate = LocationPredicate::resolve(named, registries()).unwrap();
    let baked = predicate.bake(registries(), definitions()).unwrap();
    let world = Grid {
        blocks: vec![(BlockPos::new(0, 0, 0), state(block, properties))],
        ..Grid::default()
    };
    baked.matches(
        &world,
        DVec3::splat(0.5),
        smoke(),
        &mut PredicateScratch::default(),
    )
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn state_entry_follows_property_names_and_order_pred_blk_007_009() {
    let wheat = "minecraft:wheat";
    let furnace = "minecraft:furnace";
    let cluster = "minecraft:amethyst_cluster";
    let bamboo = "minecraft:bamboo";
    let dispenser = "minecraft:dispenser";
    let dripstone = "minecraft:pointed_dripstone";
    let cases: [(&str, &str, (&str, &str), bool); 29] = [
        (
            r#"{"age": {"min": "3", "max": "5"}}"#,
            wheat,
            ("age", "2"),
            false,
        ),
        (
            r#"{"age": {"min": "3", "max": "5"}}"#,
            wheat,
            ("age", "5"),
            true,
        ),
        (r#"{"age": {"min": "+7"}}"#, wheat, ("age", "7"), true),
        (r#"{"age": {"max": "03"}}"#, wheat, ("age", "4"), false),
        (r#"{"age": {}}"#, wheat, ("age", "4"), true),
        (
            r#"{"lit": {"min": "true"}}"#,
            furnace,
            ("lit", "true"),
            true,
        ),
        (
            r#"{"lit": {"min": "true"}}"#,
            furnace,
            ("lit", "false"),
            false,
        ),
        (
            r#"{"facing": {"min": "up", "max": "north"}}"#,
            cluster,
            ("facing", "north"),
            true,
        ),
        (
            r#"{"facing": {"min": "up", "max": "north"}}"#,
            cluster,
            ("facing", "down"),
            false,
        ),
        (
            r#"{"facing": {"min": "up", "max": "north"}}"#,
            cluster,
            ("facing", "south"),
            false,
        ),
        (
            r#"{"leaves": {"min": "small"}}"#,
            bamboo,
            ("leaves", "large"),
            true,
        ),
        (
            r#"{"leaves": {"min": "small"}}"#,
            bamboo,
            ("leaves", "none"),
            false,
        ),
        (r#"{"age": {"min": "9"}}"#, wheat, ("age", "7"), false),
        (r#"{"age": {"max": "seven"}}"#, wheat, ("age", "0"), false),
        (
            r#"{"age": {"min": "5", "max": "3"}}"#,
            wheat,
            ("age", "4"),
            false,
        ),
        (
            r#"{"age": {"min": "5", "max": "3"}}"#,
            wheat,
            ("age", "5"),
            false,
        ),
        (
            r#"{"facing": {"max": "north"}}"#,
            dispenser,
            ("facing", "up"),
            true,
        ),
        (
            r#"{"facing": {"max": "north"}}"#,
            dispenser,
            ("facing", "east"),
            false,
        ),
        (
            r#"{"vertical_direction": {"max": "down"}}"#,
            dripstone,
            ("vertical_direction", "down"),
            true,
        ),
        (
            r#"{"vertical_direction": {"max": "down"}}"#,
            dripstone,
            ("vertical_direction", "up"),
            false,
        ),
        (r#"{"age": "07"}"#, wheat, ("age", "7"), true),
        (r#"{"age": "+7"}"#, wheat, ("age", "7"), true),
        (r#"{"age": "-1"}"#, wheat, ("age", "0"), false),
        (r#"{"age": "00000000000007"}"#, wheat, ("age", "7"), true),
        (r#"{"age": {"min": "07"}}"#, wheat, ("age", "7"), true),
        (r#"{"age": {"max": "-0"}}"#, wheat, ("age", "0"), true),
        (r#"{"age": {"max": "-0"}}"#, wheat, ("age", "1"), false),
        (
            r#"{"age": {"max": "2147483648"}}"#,
            wheat,
            ("age", "0"),
            false,
        ),
        (r#"{"age": {"min": "x"}}"#, wheat, ("age", "7"), false),
    ];
    for (state_json, block, property, expected) in cases {
        assert_eq!(
            passes(state_json, block, &[property]),
            expected,
            "{state_json} on {block} with {property:?}"
        );
    }
}
