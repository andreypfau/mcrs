use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_anvil::{Chunk, ErrorKind, parse_chunk};
use mcrs_minecraft_server::world::format::anvil::CorpusBlockStates;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_world::block::definition::schema::PropertyValue;
use mcrs_minecraft_world::block::definition::{BlockDefinitions, BlockEntry, load_block_definitions};

fn corpus() -> &'static BlockDefinitions {
    static CORPUS: OnceLock<BlockDefinitions> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        load_block_definitions(&asset_server)
            .expect("the corpus loads")
            .0
    })
}

fn text(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Str(s) => s.to_string(),
        PropertyValue::Int(i) => i.to_string(),
        PropertyValue::Bool(b) => b.to_string(),
    }
}

/// The property values of one of a block's states, in the corpus's own order.
/// The last property varies fastest, so the offset unwinds from the back.
fn state_properties(block: &BlockEntry, state: u16) -> Vec<(&str, &PropertyValue)> {
    let mut offset = state - block.base_state_id.0;
    let mut values = Vec::new();
    for property in block.properties.0.iter().rev() {
        let index = offset as usize % property.values.len();
        offset /= property.values.len() as u16;
        values.push((&*property.name, &property.values[index]));
    }
    values.reverse();
    values
}

fn entry(name: &str, properties: &[(&str, String)]) -> NbtTag {
    let mut entry = NbtCompound::new();
    entry.put_string("Name", name.to_string());
    if !properties.is_empty() {
        let mut props = NbtCompound::new();
        for (key, value) in properties {
            props.put_string(key, value.clone());
        }
        entry.put_component("Properties", props);
    }
    NbtTag::Compound(entry)
}

fn default_entry(block: &BlockEntry) -> NbtTag {
    let properties: Vec<(&str, String)> = state_properties(block, block.default_state_id.0)
        .iter()
        .map(|(name, value)| (*name, text(value)))
        .collect();
    entry(block.identifier.as_str(), &properties)
}

fn section(y: i8, palette: Vec<NbtTag>) -> NbtTag {
    let len = palette.len();
    let mut states = NbtCompound::new();
    states.put_list("palette", palette);
    if len > 1 {
        let bits = mcrs_voxel_storage::ceillog2(len).max(4);
        let indices: Vec<u16> = (0..4096).map(|i| (i % len) as u16).collect();
        states.put(
            "data",
            NbtTag::LongArray(
                mcrs_voxel_storage::pack_from(bits, &indices, |&i| i as u32).into_vec(),
            ),
        );
    }
    let mut section = NbtCompound::new();
    section.put_byte("Y", y);
    section.put_component("block_states", states);
    NbtTag::Compound(section)
}

fn chunk(sections: Vec<NbtTag>) -> Chunk {
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", mcrs_minecraft_anvil::DATA_VERSION);
    root.put_int("xPos", 0);
    root.put_int("zPos", 0);
    root.put_int("yPos", -4);
    root.put_string("Status", "minecraft:full".to_string());
    root.put_bool("isLightOn", true);
    root.put_long("InhabitedTime", 0);
    root.put_long("LastUpdate", 0);
    root.put_list("sections", sections);
    let bytes = mcrs_minecraft_nbt::Nbt::new(String::new(), root)
        .write()
        .to_vec();
    parse_chunk(&bytes).expect("the chunk decodes")
}

fn resolve(sections: Vec<NbtTag>) -> Result<Vec<u32>, ErrorKind> {
    let chunk = chunk(sections);
    let lookup = CorpusBlockStates(corpus());
    let mut ids = Vec::new();
    for section in &chunk.sections {
        ids.extend(
            section
                .block_states
                .as_ref()
                .expect("the section carries block states")
                .resolve_palette(&lookup)?,
        );
    }
    Ok(ids)
}

#[test]
fn every_block_default_state_round_trips_through_the_palette() {
    let definitions = corpus();
    let palette: Vec<NbtTag> = definitions.blocks().iter().map(default_entry).collect();
    let ids = resolve(vec![section(0, palette)]).expect("every default state resolves");

    let mismatched: Vec<String> = definitions
        .blocks()
        .iter()
        .zip(&ids)
        .filter(|(block, id)| **id != block.default_state_id.0 as u32)
        .map(|(block, &id)| {
            format!(
                "{} resolved to {id}, expected {}",
                block.identifier.as_str(),
                block.default_state_id.0
            )
        })
        .collect();
    assert!(
        mismatched.is_empty(),
        "{} of {} blocks disagree:\n{}",
        mismatched.len(),
        ids.len(),
        mismatched.join("\n")
    );
}

/// Every state of a block with all three property types, not just its default.
#[test]
fn every_state_of_a_stair_and_a_note_block_round_trips() {
    let definitions = corpus();
    for name in ["minecraft:oak_stairs", "minecraft:note_block"] {
        let block = definitions.block(name).unwrap();
        let states: Vec<u16> = (0..block.state_count)
            .map(|i| block.base_state_id.0 + i)
            .collect();
        let palette: Vec<NbtTag> = states
            .iter()
            .map(|&state| {
                let properties: Vec<(&str, String)> = state_properties(block, state)
                    .iter()
                    .map(|(name, value)| (*name, text(value)))
                    .collect();
                entry(name, &properties)
            })
            .collect();
        let ids = resolve(vec![section(0, palette)]).unwrap();
        assert_eq!(
            ids,
            states.iter().map(|&s| s as u32).collect::<Vec<_>>(),
            "{name}"
        );
    }
}

#[test]
fn the_typed_cases_resolve_to_the_ids_the_corpus_states() {
    let definitions = corpus();
    let cases = [
        (
            "minecraft:grass_block",
            vec![("snowy", "false")],
            vec![("snowy", PropertyValue::Bool(false))],
        ),
        (
            "minecraft:note_block",
            vec![("instrument", "harp"), ("note", "17"), ("powered", "true")],
            vec![
                ("instrument", PropertyValue::Str("harp".into())),
                ("note", PropertyValue::Int(17)),
                ("powered", PropertyValue::Bool(true)),
            ],
        ),
        (
            "minecraft:oak_stairs",
            vec![
                ("facing", "east"),
                ("half", "top"),
                ("shape", "inner_left"),
                ("waterlogged", "true"),
            ],
            vec![
                ("facing", PropertyValue::Str("east".into())),
                ("half", PropertyValue::Str("top".into())),
                ("shape", PropertyValue::Str("inner_left".into())),
                ("waterlogged", PropertyValue::Bool(true)),
            ],
        ),
        (
            "minecraft:water",
            vec![("level", "3")],
            vec![("level", PropertyValue::Int(3))],
        ),
    ];

    for (name, saved, typed) in cases {
        let block = definitions.block(name).unwrap();
        let expected = block.state_id(&typed).unwrap();
        let properties: Vec<(&str, String)> =
            saved.iter().map(|(k, v)| (*k, v.to_string())).collect();
        let ids = resolve(vec![section(0, vec![entry(name, &properties)])]).unwrap();
        println!("{name} {saved:?} -> {}", ids[0]);
        assert_eq!(ids, vec![expected.0 as u32], "{name}");
    }
}

#[test]
fn a_decoded_chunk_resolves_every_section_palette() {
    let definitions = corpus();
    let sections = vec![
        section(
            -4,
            vec![
                entry("minecraft:bedrock", &[]),
                entry("minecraft:deepslate", &[("axis", "y".into())]),
            ],
        ),
        section(0, vec![entry("minecraft:stone", &[])]),
        section(
            4,
            vec![
                entry("minecraft:air", &[]),
                entry("minecraft:water", &[("level", "0".into())]),
                entry(
                    "minecraft:oak_stairs",
                    &[
                        ("facing", "north".into()),
                        ("half", "bottom".into()),
                        ("shape", "straight".into()),
                        ("waterlogged", "true".into()),
                    ],
                ),
            ],
        ),
    ];
    let ids = resolve(sections).unwrap();
    assert_eq!(ids.len(), 6);
    assert_eq!(
        ids[0],
        definitions
            .block("minecraft:bedrock")
            .unwrap()
            .default_state_id
            .0 as u32
    );
    assert_eq!(
        ids[2],
        definitions
            .block("minecraft:stone")
            .unwrap()
            .default_state_id
            .0 as u32
    );
    assert_eq!(
        ids[3],
        definitions
            .block("minecraft:air")
            .unwrap()
            .default_state_id
            .0 as u32
    );
    assert!(
        ids.iter()
            .all(|&id| (id as usize) < definitions.state_count())
    );
}

#[test]
fn an_unknown_block_name_is_a_loud_error() {
    let err = resolve(vec![section(0, vec![entry("minecraft:unobtainium", &[])])]).unwrap_err();
    assert!(
        matches!(&err, ErrorKind::UnknownPaletteEntry { name } if name == "minecraft:unobtainium"),
        "{err}"
    );
}

#[test]
fn a_value_the_block_does_not_declare_is_a_loud_error() {
    for properties in [
        vec![
            ("facing", "up".to_string()),
            ("half", "bottom".to_string()),
            ("shape", "straight".to_string()),
            ("waterlogged", "false".to_string()),
        ],
        vec![("facing", "north".to_string())],
    ] {
        let err = resolve(vec![section(
            0,
            vec![entry("minecraft:oak_stairs", &properties)],
        )])
        .unwrap_err();
        assert!(
            matches!(&err, ErrorKind::UnknownPaletteEntry { name } if name == "minecraft:oak_stairs"),
            "{err}"
        );
    }
}
