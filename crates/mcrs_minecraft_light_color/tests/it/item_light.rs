use std::sync::OnceLock;

use mcrs_minecraft_core::TagKey;
use mcrs_minecraft_keys::Item;
use mcrs_minecraft_light_color::asset::LightColorFile;
use mcrs_minecraft_light_color::colors::LightColors;
use mcrs_minecraft_light_color::item::{ItemLight, ItemLightError, ItemLightFile, ItemLights};
use mcrs_minecraft_registry::{BlockStateId, ItemId};
use mcrs_minecraft_worldgen_testing::assets_dir;
use proptest::prelude::*;

use crate::corpus::{asset_server, block_tags, blocks, fluid_tags, item_tags, items};

fn colours() -> &'static LightColors {
    static COLOURS: OnceLock<LightColors> = OnceLock::new();
    COLOURS.get_or_init(|| {
        LightColors::load(asset_server(), blocks(), block_tags()).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn shipped() -> &'static ItemLights {
    static LIGHTS: OnceLock<ItemLights> = OnceLock::new();
    LIGHTS.get_or_init(|| {
        ItemLights::load(asset_server(), blocks(), items(), item_tags(), fluid_tags())
            .unwrap_or_else(|e| panic!("{e}"))
    })
}

fn state(block: &str) -> BlockStateId {
    blocks()
        .block(block)
        .unwrap_or_else(|| panic!("{block}"))
        .default_state_id
}

fn state_with(block: &str, properties: &[(&str, &str)]) -> BlockStateId {
    let entry = blocks().block(block).unwrap_or_else(|| panic!("{block}"));
    properties
        .iter()
        .fold(entry.default_state_id, |id, (name, value)| {
            entry
                .with_text(id, name, value)
                .unwrap_or_else(|| panic!("{block}[{name}={value}]"))
        })
}

fn light_in(item: &str, stack: &[(&str, &str)], origin: BlockStateId) -> Option<ItemLight> {
    let id = items().id_of(item).unwrap_or_else(|| panic!("{item}"));
    shipped().light(blocks(), colours(), id, stack.iter().copied(), origin)
}

fn light(item: &str, stack: &[(&str, &str)]) -> Option<ItemLight> {
    light_in(item, stack, state("minecraft:air"))
}

fn file_colour(name: &str) -> [u8; 3] {
    let path = assets_dir().join(format!("mcrs/light_color/{name}.json"));
    let file: LightColorFile = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    file.color.0
}

fn assert_emits(item: &str, emission: u8, colour: &str) {
    let light = light(item, &[]).unwrap_or_else(|| panic!("{item} emits"));
    assert_eq!(light.emission, emission, "{item}");
    assert_eq!(
        colours().rgb(light.light_type),
        Some(file_colour(colour)),
        "{item}"
    );
}

#[test]
fn item_lights_load_against_the_corpus() {
    glow_berries_emit_as_lit_cave_vines();
    an_unlit_candle_stays_dark_and_a_lit_candle_stack_glows();
    an_unlit_furnace_lamp_and_bulb_stay_dark();
    water_sensitive_items_go_dark_with_water_at_the_origin();
    other_items_keep_their_light_in_water();
    an_unknown_stack_property_or_value_is_ignored();
    an_item_in_two_files_fails();
    an_item_mapped_twice_in_one_file_fails();
    a_missing_water_tag_fails();
    an_unknown_block_property_or_value_fails();
}

fn glow_berries_emit_as_lit_cave_vines() {
    assert_emits("minecraft:glow_berries", 14, "lava");
}

fn an_unlit_candle_stays_dark_and_a_lit_candle_stack_glows() {
    assert_eq!(light("minecraft:candle", &[]), None);
    let lit = light("minecraft:candle", &[("lit", "true"), ("candles", "4")])
        .expect("four lit candles emit");
    assert_eq!(lit.emission, 12);
}

fn an_unlit_furnace_lamp_and_bulb_stay_dark() {
    for item in [
        "minecraft:furnace",
        "minecraft:redstone_lamp",
        "minecraft:copper_bulb",
    ] {
        assert_eq!(light(item, &[]), None, "{item}");
    }
    assert_eq!(
        light("minecraft:redstone_lamp", &[("lit", "true")]).map(|l| l.emission),
        Some(15)
    );
}

fn water_sensitive() -> Vec<ItemId> {
    let tag = TagKey::<Item, _>::new(mcrs_minecraft_core::rl!("mcrs:water_sensitive_light"));
    let members = item_tags().get(&tag).expect("the water-sensitive item tag");
    members.iter().map(|i| ItemId(i as u16)).collect()
}

fn light_of(item: ItemId, origin: BlockStateId) -> Option<ItemLight> {
    shipped().light(blocks(), colours(), item, std::iter::empty(), origin)
}

fn water_origins() -> [(&'static str, BlockStateId); 4] {
    [
        ("water source", state("minecraft:water")),
        (
            "flowing water",
            state_with("minecraft:water", &[("level", "3")]),
        ),
        (
            "waterlogged slab",
            state_with("minecraft:oak_slab", &[("waterlogged", "true")]),
        ),
        ("bubble column", state("minecraft:bubble_column")),
    ]
}

fn water_sensitive_items_go_dark_with_water_at_the_origin() {
    let items = water_sensitive();
    assert!(!items.is_empty());
    for item in items {
        assert!(
            light_of(item, state("minecraft:air")).is_some(),
            "{item:?} in air"
        );
        for (origin, id) in water_origins() {
            assert_eq!(light_of(item, id), None, "{item:?} in {origin}");
        }
    }
}

fn other_items_keep_their_light_in_water() {
    for item in ["minecraft:lantern", "minecraft:glowstone"] {
        let dry = light(item, &[]).expect(item);
        for (origin, id) in water_origins() {
            assert_eq!(light_in(item, &[], id), Some(dry), "{item} in {origin}");
        }
    }
}

fn an_unknown_stack_property_or_value_is_ignored() {
    assert_eq!(
        light("minecraft:torch", &[("bogus", "x")]).map(|l| l.emission),
        Some(14)
    );
    assert_eq!(light("minecraft:candle", &[("lit", "maybe")]), None);
}

fn stack_text() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(vec![
            "lit",
            "candles",
            "level",
            "berries",
            "waterlogged",
            "facing",
            "true",
            "false",
            "0",
            "4",
            "15",
            "north",
        ])
        .prop_map(String::from),
        "\\PC{0,8}",
    ]
}

fn shipped_files() -> Vec<(String, Vec<u8>)> {
    let path = "mcrs/item_light/minecraft.json";
    vec![(
        path.to_owned(),
        std::fs::read(assets_dir().join(path)).unwrap(),
    )]
}

fn edited(
    edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) -> Vec<(String, Vec<u8>)> {
    let (path, bytes) = shipped_files().remove(0);
    let mut map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&bytes).unwrap();
    edit(&mut map);
    vec![(path, serde_json::to_vec(&map).unwrap())]
}

fn load(files: Vec<(String, Vec<u8>)>) -> Result<ItemLights, ItemLightError> {
    ItemLights::from_files(files, blocks(), items(), item_tags(), fluid_tags())
}

fn an_item_in_two_files_fails() {
    let mut files = shipped_files();
    files.push((
        "mcrs/item_light/extra.json".to_owned(),
        br#"{ "minecraft:torch": "minecraft:torch" }"#.to_vec(),
    ));
    let error = load(files).unwrap_err();
    assert!(
        matches!(error, ItemLightError::DuplicateItem { .. }),
        "{error}"
    );
}

fn an_item_mapped_twice_in_one_file_fails() {
    let (path, bytes) = shipped_files().remove(0);
    let text = String::from_utf8(bytes).unwrap().replacen(
        '{',
        r#"{ "minecraft:torch": "minecraft:soul_torch","#,
        1,
    );
    let error = load(vec![(path.clone(), text.into_bytes())]).unwrap_err();
    match error {
        ItemLightError::Parse {
            path: failed,
            source,
        } => {
            assert_eq!(failed, path);
            assert!(
                source
                    .to_string()
                    .contains("`minecraft:torch` is mapped twice"),
                "{source}"
            );
        }
        other => panic!("{other}"),
    }
}

#[test]
fn the_shipped_item_map_round_trips_unchanged() {
    let text: serde_json::Value = serde_json::from_slice(
        &std::fs::read(assets_dir().join("mcrs/item_light/minecraft.json")).unwrap(),
    )
    .unwrap();
    let parsed: ItemLightFile = serde_json::from_value(text.clone()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), text);
}

fn a_missing_water_tag_fails() {
    let error = ItemLights::from_files(
        shipped_files(),
        blocks(),
        items(),
        &Default::default(),
        fluid_tags(),
    )
    .unwrap_err();
    assert!(
        matches!(&error, ItemLightError::MissingTag { tag } if tag == "mcrs:water_sensitive_light"),
        "{error}"
    );
}

fn an_unknown_block_property_or_value_fails() {
    for (target, expected) in [
        ("minecraft:no_such_block", "UnknownBlock"),
        ("minecraft:torch[lit=true]", "UnknownProperty"),
        ("minecraft:candle[lit=maybe]", "UnknownValue"),
    ] {
        let error = load(edited(|map| {
            map.insert("minecraft:candle".into(), target.into());
        }))
        .unwrap_err();
        assert!(
            format!("{error:?}").starts_with(expected),
            "{target}: {error}"
        );
    }
}

mod exhaustive {
    use super::*;

    proptest! {
        #[test]
        fn arbitrary_input_never_panics(
            item in 0..=u16::MAX,
            stack in prop::collection::vec((stack_text(), stack_text()), 0..=8),
            origin in 0..=u16::MAX,
        ) {
            let light = shipped().light(
                blocks(),
                colours(),
                ItemId(item),
                stack.iter().map(|(k, v)| (k.as_str(), v.as_str())),
                BlockStateId(origin),
            );
            prop_assert!(light.is_none_or(|l| l.emission > 0));
        }
    }
}
