use glam::DVec3;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_item::component::predicate::BlockPredicate;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_predicate::{
    BakedBlockPredicate, BakedLocationPredicate, BlockHost, LocationPredicate, LocationView,
    PredicateScratch,
};
use mcrs_minecraft_registries::{Declaration, Registries, Resolve};
use mcrs_minecraft_resource::{Id, ResourceLocationBuf};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::grid::{Grid, definitions, id, registries, smoke, state};

const AT: DVec3 = DVec3::new(0.5, 64.5, 0.5);

fn bake(json: &str) -> BakedLocationPredicate {
    let named = serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"));
    let predicate = LocationPredicate::resolve(named, registries()).unwrap();
    predicate.bake(registries(), definitions()).unwrap()
}

fn passes<W: LocationView>(json: &str, world: &W) -> bool {
    bake(json).matches(world, AT, smoke(), &mut PredicateScratch::default())
}

fn column(at: &str, below: &[BlockStateId]) -> Grid {
    let stone = state("minecraft:stone", &[]);
    let mut blocks = vec![(BlockPos::new(0, 64, 0), state(at, &[]))];
    for y in 58..64 {
        let depth = usize::try_from(63 - y).unwrap();
        blocks.push((
            BlockPos::new(0, y, 0),
            below.get(depth).copied().unwrap_or(stone),
        ));
    }
    Grid {
        blocks,
        ..Grid::default()
    }
}

fn air() -> Grid {
    column("minecraft:air", &[])
}

fn stateless() -> Grid {
    Grid {
        stateless: vec![BlockPos::new(0, 64, 0)],
        ..Grid::default()
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn each_field_of_the_location_predicate_adds_its_test_pred_loc_002() {
    let air_state = state("minecraft:air", &[]);
    let lit = state("minecraft:campfire", &[("lit", "true")]);
    let unlit = state("minecraft:campfire", &[("lit", "false")]);
    let hay = state("minecraft:hay_block", &[]);
    let first_dimension = Id::checked(0, 1).unwrap();
    let plains = id("minecraft:worldgen/biome", "minecraft:plains");
    let forest = id("minecraft:worldgen/biome", "minecraft:forest");
    let village = id("minecraft:worldgen/structure", "minecraft:village_plains");
    let water = FluidState::source_of(Fluid::Water.id());
    let mut saved = NbtCompound::new();
    saved.put_string("CustomName", "Box".to_owned());

    let cases: Vec<(&str, &str, Grid, bool)> = vec![
        (
            "PRED-LOC-002",
            r#"{"position": {"x": {"min": 0.5}}}"#,
            Grid::default(),
            true,
        ),
        (
            "PRED-LOC-002",
            r#"{"position": {"y": {"min": 64.6}}}"#,
            Grid::default(),
            false,
        ),
        (
            "PRED-LOC-002",
            r#"{"dimension": "minecraft:nowhere"}"#,
            Grid {
                dimension: Some(first_dimension),
                ..air()
            },
            false,
        ),
        (
            "PRED-LOC-002",
            r#"{"biomes": "minecraft:plains"}"#,
            Grid {
                biome: Some(plains),
                ..air()
            },
            true,
        ),
        (
            "PRED-LOC-002",
            r#"{"biomes": "minecraft:plains"}"#,
            Grid {
                biome: Some(forest),
                ..air()
            },
            false,
        ),
        (
            "PRED-LOC-002",
            r##"{"biomes": "#minecraft:is_forest"}"##,
            Grid {
                biome: Some(forest),
                ..air()
            },
            true,
        ),
        (
            "PRED-LOC-003",
            r#"{"structures": "minecraft:village_plains"}"#,
            Grid {
                structures: vec![village],
                ..air()
            },
            true,
        ),
        (
            "PRED-LOC-003",
            r##"{"structures": "#minecraft:village"}"##,
            Grid {
                structures: vec![village],
                ..air()
            },
            true,
        ),
        (
            "PRED-LOC-003",
            r#"{"structures": "minecraft:village_plains"}"#,
            air(),
            false,
        ),
        (
            "PRED-LOC-004",
            r#"{"smokey": true}"#,
            column("minecraft:air", &[air_state, air_state, lit]),
            true,
        ),
        (
            "PRED-LOC-004",
            r#"{"smokey": true}"#,
            column("minecraft:air", &[air_state, hay, lit]),
            true,
        ),
        (
            "PRED-LOC-004",
            r#"{"smokey": true}"#,
            column("minecraft:air", &[hay, air_state, lit]),
            false,
        ),
        (
            "PRED-LOC-004",
            r#"{"smokey": false}"#,
            column("minecraft:air", &[air_state, hay, unlit]),
            true,
        ),
        (
            "PRED-LOC-006",
            r#"{"light": {"light": {"min": 10}}}"#,
            Grid {
                block_light: Some(4),
                sky_light: Some(15),
                sky_light_level: Some(15.0),
                ..air()
            },
            true,
        ),
        (
            "PRED-LOC-007",
            r#"{"light": {"light": {"min": 10}}}"#,
            Grid {
                block_light: Some(4),
                sky_light: Some(15),
                sky_light_level: Some(4.5),
                ..air()
            },
            false,
        ),
        (
            "PRED-LOC-007",
            r#"{"light": {"light": 5}}"#,
            Grid {
                block_light: Some(4),
                sky_light: Some(15),
                sky_light_level: Some(4.5),
                ..air()
            },
            true,
        ),
        (
            "PRED-BLK-002",
            r#"{"block": {"blocks": "minecraft:stone"}}"#,
            column("minecraft:stone", &[]),
            true,
        ),
        (
            "PRED-BLK-002",
            r#"{"block": {"blocks": "minecraft:stone"}}"#,
            column("minecraft:dirt", &[]),
            false,
        ),
        ("PRED-BLK-001", r#"{"block": {}}"#, Grid::default(), false),
        ("PRED-GEN-005", r#"{"block": {}}"#, stateless(), true),
        (
            "PRED-GEN-004",
            r#"{"block": {"blocks": "minecraft:stone"}}"#,
            stateless(),
            false,
        ),
        (
            "PRED-BLK-003",
            r#"{"block": {"nbt": "{CustomName: \"Box\"}"}}"#,
            Grid {
                block_entity: Some(saved.clone()),
                ..stateless()
            },
            true,
        ),
        (
            "PRED-BLK-003",
            r#"{"block": {"nbt": "{CustomName: \"Box\"}"}}"#,
            Grid {
                block_entity: Some(saved.clone()),
                ..air()
            },
            true,
        ),
        (
            "PRED-BLK-003",
            r#"{"block": {"nbt": "{CustomName: \"Bag\"}"}}"#,
            Grid {
                block_entity: Some(saved),
                ..air()
            },
            false,
        ),
        ("PRED-BLK-003", r#"{"block": {"nbt": "{}"}}"#, air(), false),
        (
            "PRED-BLK-010",
            r#"{"fluid": {"fluids": "minecraft:water", "state": {"level": "8"}}}"#,
            Grid {
                fluid: Some(water),
                ..air()
            },
            true,
        ),
        (
            "PRED-BLK-010",
            r#"{"fluid": {"fluids": "minecraft:lava"}}"#,
            Grid {
                fluid: Some(water),
                ..air()
            },
            false,
        ),
        (
            "PRED-BLK-010",
            r#"{"fluid": {"fluids": "minecraft:empty"}}"#,
            air(),
            true,
        ),
        (
            "PRED-BLK-010",
            r#"{"fluid": {"state": {"source": "false"}}}"#,
            air(),
            false,
        ),
        (
            "PRED-LOC-005",
            r#"{"can_see_sky": true}"#,
            Grid {
                sky_light: Some(15),
                ..air()
            },
            true,
        ),
        (
            "PRED-LOC-005",
            r#"{"can_see_sky": true}"#,
            Grid {
                sky_light: Some(14),
                ..air()
            },
            false,
        ),
        (
            "PRED-LOC-005",
            r#"{"can_see_sky": false}"#,
            Grid {
                sky_light: Some(14),
                ..air()
            },
            true,
        ),
        (
            "PRED-LOC-005",
            r#"{"can_see_sky": false}"#,
            Grid {
                sky_light: Some(0),
                ..air()
            },
            true,
        ),
    ];
    for (rule, json, world, expected) in cases {
        assert_eq!(passes(json, &world), expected, "{rule}: {json}");
    }
}

#[test]
fn the_dimension_test_compares_the_id_that_the_world_answers_pred_loc_002() {
    let location = |text: &str| ResourceLocationBuf::read(text).unwrap();
    let names = vec![
        location("minecraft:overworld"),
        location("minecraft:the_nether"),
    ];
    let declaration = Declaration::fixed(location("minecraft:dimension"), names);
    let (dimensions, _) =
        Registries::load(vec![declaration], Default::default(), Default::default()).unwrap();
    let named = serde_json::from_str(r#"{"dimension": "minecraft:overworld"}"#).unwrap();
    let predicate = LocationPredicate::resolve(named, &dimensions).unwrap();
    let baked = predicate.bake(&dimensions, definitions()).unwrap();
    let cases: [(u16, bool); 2] = [(0, true), (1, false)];
    for (number, expected) in cases {
        let world = Grid {
            dimension: Some(Id::checked(number, 2).unwrap()),
            ..Grid::default()
        };
        let passes = baked.matches(&world, AT, smoke(), &mut PredicateScratch::default());
        assert_eq!(passes, expected, "the world answers dimension {number}");
    }
}

#[test]
fn a_block_predicate_matches_a_state_and_its_block_entity_data_pred_blk_002() {
    let stone = state("minecraft:stone", &[]);
    let dirt = state("minecraft:dirt", &[]);
    let mut saved = NbtCompound::new();
    saved.put_string("CustomName", "Box".to_owned());
    let cases: [(&str, BlockStateId, Option<&NbtCompound>, bool); 5] = [
        (r#"{"blocks": "minecraft:stone"}"#, dirt, None, false),
        (r#"{"blocks": "minecraft:stone"}"#, stone, None, true),
        (r#"{"nbt": "{CustomName: \"Box\"}"}"#, stone, None, false),
        (
            r#"{"nbt": "{CustomName: \"Box\"}"}"#,
            stone,
            Some(&saved),
            true,
        ),
        (
            r#"{"predicates": {"minecraft:damage": {}}}"#,
            stone,
            Some(&saved),
            false,
        ),
    ];
    for (json, at, block_entity, expected) in cases {
        let named = serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"));
        let predicate = BlockPredicate::resolve(named, registries()).unwrap();
        let baked = BakedBlockPredicate::bake(&predicate, registries(), definitions()).unwrap();
        let passes = baked.matches_state(Some(at), block_entity);
        assert_eq!(passes, expected, "{json}");
    }
}

#[test]
fn a_test_without_its_value_fails_pred_gen_004_and_pred_gen_005() {
    let cases: [(&str, bool); 12] = [
        ("{}", true),
        (r#"{"position": {"x": {"min": 0.0}}}"#, true),
        (r#"{"dimension": "minecraft:overworld"}"#, false),
        (r#"{"biomes": "minecraft:plains"}"#, false),
        (r#"{"structures": "minecraft:village_plains"}"#, false),
        (r#"{"smokey": false}"#, false),
        (r#"{"light": {}}"#, false),
        (r#"{"block": {}}"#, false),
        (r#"{"fluid": {}}"#, false),
        (r#"{"can_see_sky": false}"#, false),
        (r#"{"can_see_sky": true}"#, false),
        (r#"{"position": {"x": {"min": 0.0}}, "block": {}}"#, false),
    ];
    for (json, expected) in cases {
        assert_eq!(passes(json, &BlockHost::none()), expected, "{json}");
    }
}
