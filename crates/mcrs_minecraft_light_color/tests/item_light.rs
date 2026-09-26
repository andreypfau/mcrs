mod corpus;

use std::sync::OnceLock;

use corpus::{asset_server, block_tags, blocks, items};
use mcrs_minecraft_light_color::asset::LightColorFile;
use mcrs_minecraft_light_color::colors::LightColors;
use mcrs_minecraft_light_color::item::{ItemLight, ItemLights};
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_worldgen_testing::assets_dir;

fn colours() -> &'static LightColors {
    static COLOURS: OnceLock<LightColors> = OnceLock::new();
    COLOURS.get_or_init(|| {
        LightColors::load(asset_server(), blocks(), block_tags()).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn shipped() -> &'static ItemLights {
    static LIGHTS: OnceLock<ItemLights> = OnceLock::new();
    LIGHTS.get_or_init(|| {
        ItemLights::load(asset_server(), blocks(), items()).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn state(block: &str) -> BlockStateId {
    blocks()
        .block(block)
        .unwrap_or_else(|| panic!("{block}"))
        .default_state_id
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
fn a_torch_emits_like_the_placed_torch() {
    assert_emits("minecraft:torch", 14, "fire");
}

#[test]
fn glow_berries_emit_as_lit_cave_vines() {
    assert_emits("minecraft:glow_berries", 14, "lava");
}

#[test]
fn a_lava_bucket_emits_as_lava() {
    assert_emits("minecraft:lava_bucket", 15, "lava");
}

#[test]
fn a_fire_charge_emits_as_fire() {
    assert_emits("minecraft:fire_charge", 15, "fire");
}

#[test]
fn an_unlit_candle_stays_dark_and_a_lit_candle_stack_glows() {
    assert_eq!(light("minecraft:candle", &[]), None);
    let lit = light("minecraft:candle", &[("lit", "true"), ("candles", "4")])
        .expect("four lit candles emit");
    assert_eq!(lit.emission, 12);
}

#[test]
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

#[test]
fn the_shipped_map_covers_every_emitting_block_item() {
    assert_eq!(shipped().mapped_count(), 82);
}
