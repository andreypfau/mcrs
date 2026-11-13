use std::collections::HashMap;
use std::sync::LazyLock;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{BlockDefinitions, build_block_definitions};
use mcrs_minecraft_block_support::{BlockSupport, SupportIssue};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_voxel::{BlockPos, BlockStateId};
use serde_json::{Value, json};

const P: BlockPos = BlockPos::new(0, 64, 0);

/// The vanilla corpus with the components of block-support.md section 5 that the next recapture
/// writes. The plants keep the short form the corpus holds; `short_grass` also gets the tested
/// directions of R4, and `lilac` has its filter only on its lower part.
static WORLD: LazyLock<(BlockDefinitions, BlockSupport, Vec<SupportIssue>)> = LazyLock::new(|| {
    let below = |predicate: Value| json!({ "predicate": predicate });
    let face_below = |test: &str| json!({"type": "minecraft:has_sturdy_face", "offset": [0, -1, 0], "direction": "up", "face_test": test});
    let block_wide: HashMap<&str, Value> = HashMap::from([
        (
            "minecraft:torch",
            json!({"minecraft:placement_filter": below(face_below("center"))}),
        ),
        (
            "minecraft:brown_mushroom",
            json!({"minecraft:placement_filter": {
            "predicate": face_below("center"), "tested_directions": "all"}}),
        ),
        (
            "minecraft:leaf_litter",
            json!({"minecraft:placement_filter": {
            "predicate": face_below("full"), "tested_directions": "all"}}),
        ),
        (
            "minecraft:white_carpet",
            json!({"minecraft:placement_filter": {"tested_directions": "all",
            "predicate": {"type": "minecraft:not", "predicate":
                {"type": "minecraft:matching_block_tag", "offset": [0, -1, 0], "tag": "minecraft:air"}}}}),
        ),
        (
            "minecraft:stone_pressure_plate",
            json!({
            "minecraft:placement_filter": below(json!({"type": "minecraft:any_of", "predicates": [face_below("ring"), face_below("center")]})),
            "mcrs:solid": true}),
        ),
        (
            "minecraft:oak_sign",
            json!({"minecraft:placement_filter": below(
            json!({"type": "minecraft:solid", "offset": [0, -1, 0]}))}),
        ),
        (
            "minecraft:snow",
            json!({"minecraft:placement_filter": {"tested_directions": "all", "predicate":
            {"type": "minecraft:all_of", "predicates": [
                {"type": "minecraft:not", "predicate": {"type": "minecraft:matching_block_tag", "offset": [0, -1, 0], "tag": "minecraft:cannot_support_snow_layer"}},
                {"type": "minecraft:any_of", "predicates": [
                    {"type": "minecraft:matching_block_tag", "offset": [0, -1, 0], "tag": "minecraft:support_override_snow_layer"},
                    {"type": "minecraft:has_sturdy_face", "offset": [0, -1, 0], "direction": "up", "shape": "collision"},
                    {"type": "minecraft:matching_blocks", "offset": [0, -1, 0], "blocks": "minecraft:snow", "state": {"layers": "8"}}
                ]}
            ]}}}),
        ),
        (
            "minecraft:oak_leaves",
            json!({"minecraft:support": {"boxes": []}}),
        ),
        (
            "minecraft:short_grass",
            json!({"minecraft:placement_filter": {"tested_directions": "all",
            "conditions": [{"allowed_faces": ["up"], "block_filter": ["#minecraft:supports_vegetation"]}]}}),
        ),
    ]);
    let on_wall = |facing: &str| {
        let normal = Direction::all()
            .into_iter()
            .find(|direction| direction.name() == facing)
            .expect("a direction")
            .normal();
        json!({
            "condition": format!("q.block_state('facing') == '{facing}'"),
            "components": {"minecraft:placement_filter": {"predicate": {"type": "minecraft:has_sturdy_face",
                "offset": [-normal.x, 0, -normal.z], "direction": facing}}}
        })
    };
    let solid_wall = |facing: &str| {
        let mut permutation = on_wall(facing);
        let predicate = &mut permutation["components"]["minecraft:placement_filter"]["predicate"];
        let offset = predicate["offset"].clone();
        *predicate = json!({"type": "minecraft:solid", "offset": offset});
        permutation
    };
    let facings = ["north", "south", "west", "east"];
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/block_definition"
    );
    let files = std::fs::read_dir(directory)
        .expect("the corpus")
        .map(|entry| entry.expect("a corpus file").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            let mut file: Value =
                serde_json::from_slice(&std::fs::read(&path).expect("a corpus file"))
                    .expect("json");
            let block = &mut file["minecraft:block"];
            let name = block["description"]["identifier"]
                .as_str()
                .expect("an identifier")
                .to_owned();
            if let Some(Value::Object(added)) = block_wide.get(name.as_str()) {
                let components = block["components"].as_object_mut().expect("components");
                components.extend(added.clone());
            }
            let walls: Option<Vec<Value>> = match name.as_str() {
                "minecraft:wall_torch" => Some(facings.map(on_wall).to_vec()),
                "minecraft:oak_wall_sign" => Some(facings.map(solid_wall).to_vec()),
                _ => None,
            };
            if let Some(walls) = walls {
                let permutations = block["permutations"].as_array_mut().expect("permutations");
                permutations.extend(walls);
            }
            if name == "minecraft:lilac" {
                let components = block["components"].as_object_mut().expect("components");
                let filter = components
                    .remove("minecraft:placement_filter")
                    .expect("a filter");
                let permutations = block["permutations"].as_array_mut().expect("permutations");
                permutations.push(json!({
                    "condition": "q.block_state('half') == 'lower'",
                    "components": {"minecraft:placement_filter": filter},
                }));
            }
            (
                path.display().to_string(),
                serde_json::to_vec(&file).expect("json"),
            )
        });
    let names: Vec<&str> = Block::ALL
        .iter()
        .map(|block| block.as_static_str())
        .collect();
    let (definitions, _) = build_block_definitions(&names, files).expect("the patched corpus");
    let (support, issues) = BlockSupport::bake(&definitions, test_registries());
    (definitions, support, issues)
});

fn definitions() -> &'static BlockDefinitions {
    &WORLD.0
}

fn support() -> &'static BlockSupport {
    &WORLD.1
}

fn state(text: &str) -> BlockStateId {
    let (name, properties) = text.split_once('[').unwrap_or((text, ""));
    let entry = definitions()
        .block(&format!("minecraft:{name}"))
        .unwrap_or_else(|| panic!("{name}"));
    properties
        .trim_end_matches(']')
        .split(',')
        .filter(|pair| !pair.is_empty())
        .fold(entry.default_state_id, |state, pair| {
            let (property, value) = pair.split_once('=').expect("property=value");
            entry
                .with_text(state, property, value)
                .unwrap_or_else(|| panic!("{text}"))
        })
}

/// A world of air that holds `blocks` around P.
fn world(blocks: &[(Direction, &str)]) -> impl FnMut(BlockPos) -> BlockStateId + use<> {
    let placed: Vec<(BlockPos, BlockStateId)> = blocks
        .iter()
        .map(|(direction, block)| (P + direction.normal(), state(block)))
        .collect();
    let air = state("air");
    move |pos| {
        placed
            .iter()
            .find(|(at, _)| *at == pos)
            .map_or(air, |(_, state)| *state)
    }
}

#[test]
fn the_vanilla_predicates_read_no_name_the_data_pack_lacks() {
    assert_eq!(WORLD.2, []);
}

#[test]
fn a_subject_stays_on_the_bases_its_predicate_accepts() {
    use Direction::{Down, East, West};
    let cases: &[(&str, &[(Direction, &str)], bool)] = &[
        ("short_grass", &[(Down, "dirt")], true),
        ("short_grass", &[(Down, "stone")], false),
        ("poppy", &[(Down, "grass_block")], true),
        ("wither_rose", &[(Down, "soul_sand")], true),
        ("wither_rose", &[(Down, "sand")], false),
        ("torch", &[(Down, "stone")], true),
        ("torch", &[(Down, "oak_fence")], true),
        ("torch", &[(Down, "oak_slab")], false),
        ("torch", &[(Down, "oak_slab[type=top]")], true),
        ("torch", &[(Down, "oak_leaves")], false),
        ("torch", &[], false),
        ("brown_mushroom", &[(Down, "cobblestone_wall")], true),
        ("leaf_litter", &[(Down, "stone")], true),
        ("leaf_litter", &[(Down, "oak_fence")], false),
        ("wall_torch[facing=east]", &[(West, "stone")], true),
        ("wall_torch[facing=east]", &[(East, "stone")], false),
        ("wall_torch[facing=east]", &[(West, "oak_fence")], false),
        ("snow", &[(Down, "stone")], true),
        ("snow", &[(Down, "oak_leaves")], true),
        ("snow", &[(Down, "ice")], false),
        ("snow", &[(Down, "soul_sand")], true),
        ("snow", &[(Down, "snow[layers=8]")], true),
        ("snow", &[(Down, "snow[layers=7]")], false),
        ("white_carpet", &[(Down, "glass_pane")], true),
        ("white_carpet", &[(Down, "cave_air")], false),
        ("stone_pressure_plate", &[(Down, "stone")], true),
        ("stone_pressure_plate", &[(Down, "oak_fence")], true),
        ("stone_pressure_plate", &[(Down, "oak_slab")], false),
        ("oak_sign", &[(Down, "stone")], true),
        ("oak_sign", &[(Down, "stone_pressure_plate")], true),
        ("oak_sign", &[(Down, "short_grass")], false),
        ("oak_sign", &[(Down, "snow[layers=8]")], true),
        ("oak_wall_sign[facing=east]", &[(West, "oak_fence")], true),
        ("oak_wall_sign[facing=east]", &[(Down, "stone")], false),
        ("stone", &[], true),
    ];
    for (subject, blocks, expected) in cases {
        let stays = support().can_stay(state(subject), P, world(blocks));
        assert_eq!(stays, *expected, "{subject} on {blocks:?}");
    }
}

#[test]
fn a_shape_update_runs_the_test_only_from_a_tested_direction() {
    let unsupported = |subject| {
        Direction::all()
            .map(|from| support().stays_after_update(state(subject), P, from, world(&[])))
    };
    // In the order down, up, north, south, west, east.
    let cases = [
        ("torch", [false, true, true, true, true, true]),
        ("poppy", [false, true, true, true, true, true]),
        ("short_grass", [false; 6]),
        ("snow", [false; 6]),
        (
            "wall_torch[facing=east]",
            [true, true, true, true, false, true],
        ),
        (
            "oak_wall_sign[facing=east]",
            [true, true, true, true, false, true],
        ),
    ];
    for (subject, expected) in cases {
        assert_eq!(unsupported(subject), expected, "{subject}");
    }
}

#[test]
fn each_part_of_a_tall_plant_stays_while_the_other_part_and_the_base_do() {
    use Direction::{Down, Up, West};
    let lower = "sunflower[half=lower]";
    let upper = "sunflower[half=upper]";
    let cases: &[(&str, Direction, &[(Direction, &str)], bool)] = &[
        (lower, Down, &[(Down, "dirt"), (Up, upper)], true),
        (lower, Down, &[(Down, "stone"), (Up, upper)], false),
        (lower, Up, &[(Down, "dirt")], false),
        (
            lower,
            Up,
            &[(Down, "dirt"), (Up, "lilac[half=upper]")],
            false,
        ),
        (lower, West, &[], true),
        (upper, Down, &[(Down, lower)], true),
        (upper, Down, &[(Down, "dirt")], false),
        (upper, Up, &[], true),
    ];
    for (part, from, blocks, expected) in cases {
        let stays = support().stays_after_update(state(part), P, *from, world(blocks));
        assert_eq!(stays, *expected, "{part} from {from} among {blocks:?}");
    }
    assert!(support().is_two_part(state(lower)));
    assert!(!support().is_two_part(state("poppy")));
    assert!(support().can_stay(state(lower), P, world(&[(Down, "dirt")])));
}

#[test]
fn the_upper_part_of_a_tall_plant_goes_when_only_its_lower_part_has_a_filter() {
    use Direction::Down;
    let upper = state("lilac[half=upper]");
    let alone = support().stays_after_update(upper, P, Down, world(&[(Down, "dirt")]));
    let held = support().stays_after_update(upper, P, Down, world(&[(Down, "lilac[half=lower]")]));
    assert_eq!([alone, held], [false, true]);
}
