use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_light_color::asset::LightColorFile;
use mcrs_minecraft_light_color::colors::{LightColorError, LightColors, LightType};
use mcrs_minecraft_world::registry_load::test_registries;
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files};

use crate::corpus::{asset_server, blocks};
use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use mcrs_minecraft_light_color::item::ItemLights;
use mcrs_minecraft_light_color::plugin::LightColorPlugin;
use mcrs_minecraft_registries::CurrentRegistries;

fn block(id: &str) -> &'static BlockEntry {
    blocks()
        .block(id)
        .unwrap_or_else(|| panic!("the corpus has no `{id}`"))
}

fn states(block: &BlockEntry) -> impl Iterator<Item = BlockStateId> + '_ {
    (0..block.state_count).filter_map(|offset| block.state_at(offset))
}

fn voxel(state: BlockStateId) -> VoxelId {
    VoxelId(state.number())
}

fn load(files: &[(&str, &str)]) -> Result<LightColors, LightColorError> {
    let files = files
        .iter()
        .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()));
    LightColors::from_files(
        files,
        blocks(),
        &test_registries()
            .tags::<Block>()
            .expect("the load builds the block tags"),
    )
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
fn light_colours_load_against_the_block_corpus() {
    shipped_colours_are_uniform_per_block_and_only_on_emitters();
    an_unknown_block_fails_naming_the_file();
    a_candle_predicate_colours_only_the_lit_states();
    a_state_claimed_twice_fails_naming_both_files();
    more_than_255_colours_fail();
    the_plugin_holds_the_colour_table_and_item_lights_after_the_first_update();
}

fn the_plugin_holds_the_colour_table_and_item_lights_after_the_first_update() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        },
    ));
    app.insert_resource(blocks().clone());
    app.insert_resource(crate::corpus::items().clone());
    app.insert_resource(CurrentRegistries::new(test_registries().clone()));
    app.add_plugins(LightColorPlugin);
    app.update();
    let colors = app
        .world()
        .get_resource::<LightColors>()
        .expect("the colour table after the first update");
    assert_eq!(colors.type_count(), 14, "13 colours and the default");
    assert!(
        app.world().get_resource::<ItemLights>().is_some(),
        "the item light table after the first update"
    );
}

fn shipped_colours_are_uniform_per_block_and_only_on_emitters() {
    let colours = LightColors::load(
        asset_server(),
        blocks(),
        &test_registries()
            .tags::<Block>()
            .expect("the load builds the block tags"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut coloured = 0;
    for block in blocks().blocks() {
        let id = block.identifier.as_str();
        let first = colours.rgb(colours.light_type(voxel(block.base_state_id)));
        let mut emits = false;
        for state in states(block) {
            assert_eq!(
                colours.rgb(colours.light_type(voxel(state))),
                first,
                "{id} state {}",
                state.number()
            );
            emits |= blocks().state(state).light_emission > 0;
        }
        if first.is_some() {
            assert!(emits, "{id} is coloured but emits no light");
            coloured += 1;
        }
    }
    assert!(coloured > 0);
}

fn an_unknown_block_fails_naming_the_file() {
    let (error, message) = fails(&[("pack/glow.json", &file(&["minecraft:glowing_stone"]))]);
    assert!(
        matches!(error, LightColorError::UnknownBlock { .. }),
        "{error:?}"
    );
    assert!(message.contains("pack/glow.json"), "{message}");
    assert!(message.contains("minecraft:glowing_stone"), "{message}");
}

fn a_candle_predicate_colours_only_the_lit_states() {
    let colours = load(&[("pack/glow.json", &file(&["minecraft:candle[lit=true]"]))]).unwrap();
    let candle = block("minecraft:candle");
    for state in states(candle) {
        let lit = candle.value_of(state, "lit").unwrap().renders_to("true");
        assert_eq!(
            colours.light_type(voxel(state)) != LightType::DEFAULT,
            lit,
            "state {}",
            state.number()
        );
    }
}

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
    for path in json_files(&assets_dir().join("mcrs/light_color")) {
        let text: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let parsed: LightColorFile = serde_json::from_value(text.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            text,
            "{}",
            path.display()
        );
    }
}
