mod corpus;

use std::collections::HashMap;
use std::sync::OnceLock;

use corpus::{asset_server, block_tags, blocks};
use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_light_color::asset::LightColorFile;
use mcrs_minecraft_light_color::colors::{LightColorError, LightColors, LightType};
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files};

fn shipped() -> &'static LightColors {
    static COLORS: OnceLock<LightColors> = OnceLock::new();
    COLORS.get_or_init(|| {
        LightColors::load(asset_server(), blocks(), block_tags()).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn block(id: &str) -> &'static BlockEntry {
    blocks()
        .block(id)
        .unwrap_or_else(|| panic!("the corpus has no `{id}`"))
}

fn states(block: &BlockEntry) -> impl Iterator<Item = VoxelId> + '_ {
    (0..block.state_count).map(|offset| VoxelId(block.base_state_id.0 + offset))
}

fn colour_of_every_state(id: &str) -> Option<[u8; 3]> {
    let colors = shipped();
    let block = block(id);
    let first = colors.rgb(colors.light_type(block.base_state_id.into()));
    for state in states(block) {
        assert_eq!(
            colors.rgb(colors.light_type(state)),
            first,
            "{id} state {}",
            state.0
        );
    }
    first
}

#[test]
fn soul_file_colours_every_soul_emitter() {
    let soul = Some([0x36, 0xd9, 0xe6]);
    assert_eq!(colour_of_every_state("minecraft:soul_lantern"), soul);
    assert_eq!(
        colour_of_every_state("minecraft:calibrated_sculk_sensor"),
        soul
    );
    let glowstone = block("minecraft:glowstone").default_state_id;
    assert_eq!(shipped().light_type(glowstone.into()), LightType::DEFAULT);
}

const TABLE: &[(u32, &[&str])] = &[
    (
        0xf39a5e,
        &["torch", "wall_torch", "lantern", "fire", "campfire"],
    ),
    (
        0xe8c398,
        &[
            "exposed_copper_lantern",
            "weathered_copper_lantern",
            "oxidized_copper_lantern",
            "waxed_copper_lantern",
            "waxed_exposed_copper_lantern",
            "waxed_weathered_copper_lantern",
            "waxed_oxidized_copper_lantern",
            "redstone_lamp",
            "copper_bulb",
            "exposed_copper_bulb",
            "weathered_copper_bulb",
            "oxidized_copper_bulb",
            "waxed_copper_bulb",
            "waxed_exposed_copper_bulb",
            "waxed_weathered_copper_bulb",
            "waxed_oxidized_copper_bulb",
            "firefly_bush",
        ],
    ),
    (
        0x86ca59,
        &["copper_torch", "copper_wall_torch", "copper_lantern"],
    ),
    (
        0x36d9e6,
        &[
            "soul_lantern",
            "soul_torch",
            "soul_wall_torch",
            "soul_campfire",
            "soul_fire",
            "sculk_catalyst",
            "sculk_sensor",
            "calibrated_sculk_sensor",
        ],
    ),
    (
        0xc32c1f,
        &[
            "redstone_torch",
            "redstone_wall_torch",
            "redstone_ore",
            "deepslate_redstone_ore",
        ],
    ),
    (
        0xd3852b,
        &[
            "#candles",
            "#candle_cakes",
            "jack_o_lantern",
            "lava",
            "lava_cauldron",
            "shroomlight",
            "cave_vines",
            "cave_vines_plant",
            "vault",
            "trial_spawner",
            "magma_block",
        ],
    ),
    (0xda73de, &["end_rod", "pearlescent_froglight"]),
    (0xb0dad3, &["sea_pickle", "sea_lantern"]),
    (0xf9efa5, &["ochre_froglight"]),
    (0xb7f1bc, &["verdant_froglight", "glow_lichen"]),
    (0x6c70b2, &["end_gateway", "end_portal"]),
    (
        0xa233eb,
        &[
            "respawn_anchor",
            "nether_portal",
            "crying_obsidian",
            "enchanting_table",
        ],
    ),
    (
        0xb966e8,
        &[
            "amethyst_cluster",
            "small_amethyst_bud",
            "medium_amethyst_bud",
            "large_amethyst_bud",
        ],
    ),
];

const DEFAULT_EMITTERS: &[&str] = &[
    "beacon",
    "blast_furnace",
    "brewing_stand",
    "brown_mushroom",
    "conduit",
    "dragon_egg",
    "end_portal_frame",
    "ender_chest",
    "furnace",
    "glowstone",
    "light",
    "smoker",
];

fn rgb(hex: u32) -> [u8; 3] {
    let [_, r, g, b] = hex.to_be_bytes();
    [r, g, b]
}

fn expected() -> HashMap<String, [u8; 3]> {
    let mut expected = HashMap::new();
    for &(hex, members) in TABLE {
        for member in members {
            let ids: Vec<String> = match member.strip_prefix('#') {
                Some(tag) => {
                    let key = TagKey::<Block, _>::from_location(ResourceLocation::minecraft(tag));
                    block_tags()
                        .get(&key)
                        .unwrap_or_else(|| panic!("no tag `{tag}`"))
                        .iter()
                        .map(|i| blocks().blocks()[i as usize].identifier.to_string())
                        .collect()
                }
                None => vec![format!("minecraft:{member}")],
            };
            for id in ids {
                assert!(
                    expected.insert(id.clone(), rgb(hex)).is_none(),
                    "{id} twice"
                );
            }
        }
    }
    expected
}

fn emits(block: &BlockEntry) -> bool {
    states(block).any(|state| blocks().state(state.into()).light_emission > 0)
}

#[test]
fn shipped_files_hold_exactly_the_vibrant_visuals_table() {
    assert_eq!(shipped().type_count(), 14, "13 colours and the default");
    let expected = expected();
    assert_eq!(expected.len(), 97);
    let mut defaults = Vec::new();
    let mut coloured = 0;
    for block in blocks().blocks().iter().filter(|block| emits(block)) {
        let id = block.identifier.as_str();
        match colour_of_every_state(id) {
            Some(colour) => {
                assert_eq!(expected.get(id), Some(&colour), "{id}");
                coloured += 1;
            }
            None => {
                assert!(!expected.contains_key(id), "{id} lost its colour");
                defaults.push(id.strip_prefix("minecraft:").unwrap());
            }
        }
    }
    assert_eq!(coloured, expected.len(), "every listed block emits");
    defaults.sort_unstable();
    assert_eq!(defaults, DEFAULT_EMITTERS);
}

#[test]
fn lava_cauldron_and_calibrated_sculk_sensor_share_their_parents_colour() {
    assert_eq!(
        colour_of_every_state("minecraft:lava_cauldron"),
        colour_of_every_state("minecraft:lava")
    );
    assert_eq!(
        colour_of_every_state("minecraft:calibrated_sculk_sensor"),
        colour_of_every_state("minecraft:sculk_sensor")
    );
}

#[test]
fn only_the_bare_copper_lantern_is_green() {
    assert_eq!(
        colour_of_every_state("minecraft:copper_lantern"),
        Some(rgb(0x86ca59))
    );
    for id in [
        "exposed_copper_lantern",
        "weathered_copper_lantern",
        "oxidized_copper_lantern",
        "waxed_copper_lantern",
        "waxed_exposed_copper_lantern",
        "waxed_weathered_copper_lantern",
        "waxed_oxidized_copper_lantern",
    ] {
        assert_eq!(
            colour_of_every_state(&format!("minecraft:{id}")),
            Some(rgb(0xe8c398)),
            "{id}"
        );
    }
}

fn load(files: &[(&str, &str)]) -> Result<LightColors, LightColorError> {
    let files = files
        .iter()
        .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()));
    LightColors::from_files(files, blocks(), block_tags())
}

fn file(entries: &[&str]) -> String {
    serde_json::json!({ "color": "#36d9e6", "blocks": entries }).to_string()
}

fn fails(files: &[(&str, &str)]) -> (LightColorError, String) {
    let error = load(files).expect_err("the files load");
    let message = error.to_string();
    (error, message)
}

#[test]
fn an_unknown_block_fails_naming_the_file() {
    let (error, message) = fails(&[("pack/glow.json", &file(&["minecraft:glowing_stone"]))]);
    assert!(
        matches!(error, LightColorError::UnknownBlock { .. }),
        "{error:?}"
    );
    assert!(message.contains("pack/glow.json"), "{message}");
    assert!(message.contains("minecraft:glowing_stone"), "{message}");
}

#[test]
fn an_unknown_tag_fails() {
    let (error, message) = fails(&[("pack/glow.json", &file(&["#minecraft:glowing"]))]);
    assert!(
        matches!(error, LightColorError::UnknownTag { .. }),
        "{error:?}"
    );
    assert!(message.contains("pack/glow.json"), "{message}");
}

#[test]
fn an_undeclared_property_fails() {
    let (error, message) = fails(&[("pack/glow.json", &file(&["minecraft:torch[lit=true]"]))]);
    assert!(
        matches!(error, LightColorError::UnknownProperty { .. }),
        "{error:?}"
    );
    assert!(message.contains("pack/glow.json"), "{message}");
}

#[test]
fn an_undeclared_value_fails() {
    let (error, message) = fails(&[("pack/glow.json", &file(&["minecraft:candle[lit=maybe]"]))]);
    assert!(
        matches!(error, LightColorError::UnknownValue { .. }),
        "{error:?}"
    );
    assert!(message.contains("pack/glow.json"), "{message}");
}

#[test]
fn a_malformed_colour_fails() {
    let text = r##"{ "color": "#+6d9e6", "blocks": ["minecraft:torch"] }"##;
    let (error, message) = fails(&[("pack/glow.json", text)]);
    assert!(matches!(error, LightColorError::Parse { .. }), "{error:?}");
    assert!(message.contains("pack/glow.json"), "{message}");
}

#[test]
fn a_malformed_predicate_fails() {
    let (error, message) = fails(&[("pack/glow.json", &file(&["minecraft:candle[lit]"]))]);
    assert!(matches!(error, LightColorError::Parse { .. }), "{error:?}");
    assert!(message.contains("pack/glow.json"), "{message}");
}

#[test]
fn an_entry_with_no_emitting_state_fails() {
    for entry in ["minecraft:stone", "minecraft:candle[lit=false]"] {
        let (error, message) = fails(&[("pack/glow.json", &file(&[entry]))]);
        assert!(
            matches!(error, LightColorError::NoEmittingState { .. }),
            "{error:?}"
        );
        assert!(message.contains("pack/glow.json"), "{message}");
        assert!(message.contains(entry), "{message}");
    }
}

#[test]
fn a_candle_predicate_colours_only_the_lit_states() {
    let colours = load(&[("pack/glow.json", &file(&["minecraft:candle[lit=true]"]))]).unwrap();
    let candle = block("minecraft:candle");
    for state in states(candle) {
        let lit = candle
            .value_of(state.into(), "lit")
            .unwrap()
            .renders_to("true");
        assert_eq!(
            colours.light_type(state) != LightType::DEFAULT,
            lit,
            "state {}",
            state.0
        );
    }
}

#[test]
fn a_state_claimed_twice_fails_naming_both_files() {
    let (error, message) = fails(&[
        ("pack/candles.json", &file(&["#minecraft:candles"])),
        ("pack/lit.json", &file(&["minecraft:candle[lit=true]"])),
    ]);
    assert!(
        matches!(error, LightColorError::TwoColours { .. }),
        "{error:?}"
    );
    assert!(message.contains("pack/candles.json"), "{message}");
    assert!(message.contains("pack/lit.json"), "{message}");
}

#[test]
fn more_than_255_colours_fail() {
    let empty = r##"{ "color": "#36d9e6", "blocks": [] }"##;
    let paths: Vec<String> = (0..256).map(|i| format!("pack/{i:03}.json")).collect();
    let files: Vec<(&str, &str)> = paths.iter().map(|path| (path.as_str(), empty)).collect();
    let (error, _) = fails(&files);
    assert!(
        matches!(error, LightColorError::TooManyColours { count: 256 }),
        "{error:?}"
    );
    assert_eq!(load(&files[..255]).unwrap().type_count(), 256);
}

#[test]
fn every_shipped_file_round_trips_unchanged() {
    let files = json_files(&assets_dir().join("mcrs/light_color"));
    assert_eq!(files.len(), 13);
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let parsed: LightColorFile = serde_json::from_str(&text).unwrap();
        let written = serde_json::to_string_pretty(&parsed).unwrap() + "\n";
        assert_eq!(written, text, "{}", path.display());
    }
}
