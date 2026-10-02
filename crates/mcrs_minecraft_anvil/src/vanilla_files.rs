use mcrs_minecraft_chunk::SectionKind;
use mcrs_minecraft_chunk::section::{Biomes, NoiseBiomes};
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::tests::write::{Named, assert_same_chunk, root};
use crate::{ChunkStatus, PaletteNames, RetroGen, parse_chunk, write_chunk};

const FULL: &[u8] = include_bytes!("fixtures/vanilla/chunk_full.nbt");
const TERRAIN: &[u8] = include_bytes!("fixtures/vanilla/chunk_terrain.nbt");
const RETROGEN: &[u8] = include_bytes!("fixtures/vanilla/chunk_retrogen.nbt");
const RETROGEN_MINIMAL: &[u8] = include_bytes!("fixtures/vanilla/chunk_retrogen_minimal.nbt");

fn load(bytes: &[u8]) -> Named {
    let (blocks, biomes) = (PaletteNames::new(), PaletteNames::new());
    let chunk = parse_chunk(bytes, &blocks, &biomes).unwrap();
    Named {
        chunk,
        blocks,
        biomes,
    }
}

fn biome(loaded: &Named, name: &str) -> u8 {
    loaded.biomes.intern(name, std::iter::empty()).unwrap()
}

fn section(loaded: &Named, y: i8) -> &crate::Section {
    loaded.chunk.sections.iter().find(|s| s.y == y).unwrap()
}

fn cells(side: usize) -> impl Iterator<Item = (usize, usize, usize)> {
    (0..side).flat_map(move |y| (0..side).flat_map(move |z| (0..side).map(move |x| (x, y, z))))
}

#[test]
fn a_full_chunk_written_by_the_game_loads() {
    let loaded = load(FULL);
    let chunk = &loaded.chunk;
    assert_eq!(chunk.pos, ColumnPos::new(3, -2));
    assert_eq!(chunk.min_section_y, -4);
    assert_eq!(chunk.sections.len(), 24);
    assert_eq!(chunk.status, ChunkStatus::Full);
    assert_eq!(chunk.retrogen, None);
    assert_eq!((chunk.last_update, chunk.inhabited_time), (100, 7));
    assert!(chunk.is_light_on);
    assert!(chunk.sections.iter().all(|s| s.noise_biomes.is_none()));

    let plains = biome(&loaded, "minecraft:plains");
    let forest = biome(&loaded, "minecraft:forest");
    let desert = biome(&loaded, "minecraft:desert");

    let ground = section(&loaded, 0);
    let states = ground.block_states.as_ref().unwrap();
    let log = loaded
        .blocks
        .intern("minecraft:oak_log", [("axis", "x")])
        .unwrap();
    let water = loaded
        .blocks
        .intern("minecraft:water", [("level", "3")])
        .unwrap();
    let stone = loaded
        .blocks
        .intern("minecraft:stone", std::iter::empty())
        .unwrap();
    assert_eq!(states.get(1, 2, 3), log);
    assert_eq!(states.get(4, 5, 6), water);
    assert_eq!(states.get(0, 0, 0), stone);

    let biomes = ground.biomes.as_ref().unwrap();
    assert_eq!(biomes.get(5, 9, 13), forest);
    for (x, y, z) in [
        (4, 9, 13),
        (6, 9, 13),
        (5, 8, 13),
        (5, 10, 13),
        (5, 9, 12),
        (5, 9, 14),
    ] {
        assert_eq!(biomes.get(x, y, z), plains, "{x} {y} {z}");
    }

    let layered = section(&loaded, 1).biomes.as_ref().unwrap();
    for (y, expected) in [
        (0, plains),
        (4, plains),
        (5, forest),
        (10, forest),
        (11, desert),
        (15, desert),
    ] {
        assert_eq!(layered.get(7, y, 2), expected, "layer {y}");
    }
}

#[test]
fn a_terrain_proto_chunk_keeps_its_noise_biomes() {
    let loaded = load(TERRAIN);
    assert_eq!(loaded.chunk.status, ChunkStatus::Terrain);
    let plains = biome(&loaded, "minecraft:plains");
    let forest = biome(&loaded, "minecraft:forest");
    let desert = biome(&loaded, "minecraft:desert");
    assert_eq!(NoiseBiomes::ENTRY_COUNT, 64);
    assert_eq!(Biomes::ENTRY_COUNT, 4096);

    for section in &loaded.chunk.sections {
        let noise = section
            .noise_biomes
            .as_ref()
            .expect("a noise biome container");
        for (x, y, z) in cells(NoiseBiomes::SIZE) {
            let expected = if section.y == 0 && (x, y, z) == (1, 2, 3) {
                forest
            } else {
                plains
            };
            assert_eq!(noise.get(x, y, z), expected, "{} {x} {y} {z}", section.y);
        }
        let biomes = section.biomes.as_ref().expect("a biome container");
        for (x, y, z) in cells(Biomes::SIZE) {
            let expected = match section.y {
                0 if (x, y, z) == (5, 9, 13) => forest,
                1 if (5..11).contains(&y) => forest,
                1 if y >= 11 => desert,
                _ => plains,
            };
            assert_eq!(biomes.get(x, y, z), expected, "{} {x} {y} {z}", section.y);
        }
    }
}

#[test]
fn a_chunk_with_retrogen_loads_its_record() {
    let loaded = load(RETROGEN);
    assert_eq!(loaded.chunk.status, ChunkStatus::NoiseBiomes);
    assert_eq!(
        loaded.chunk.retrogen,
        Some(RetroGen {
            target_status: ChunkStatus::Full,
            statuses_to_rerun: vec![ChunkStatus::Biomes],
            has_below_zero_retrogen: true,
            missing_bedrock: vec![5, 0, 9],
        })
    );

    let minimal = load(RETROGEN_MINIMAL);
    assert_eq!(
        minimal.chunk.retrogen,
        Some(RetroGen {
            target_status: ChunkStatus::Full,
            statuses_to_rerun: vec![ChunkStatus::Biomes],
            has_below_zero_retrogen: false,
            missing_bedrock: vec![],
        })
    );
}

fn sorted(tag: &NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(compound) => {
            let mut entries: Vec<(String, NbtTag)> = compound
                .child_tags
                .iter()
                .map(|(key, value)| (key.clone(), sorted(value)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            NbtTag::Compound(NbtCompound {
                child_tags: entries,
            })
        }
        NbtTag::List(items) => NbtTag::List(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

fn keys(compound: &NbtCompound) -> Vec<&str> {
    let mut keys: Vec<&str> = compound
        .child_tags
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    keys.sort_unstable();
    keys
}

const SAME_AS_THE_GAME: [&str; 9] = [
    "DataVersion",
    "InhabitedTime",
    "LastUpdate",
    "isLightOn",
    "retrogen",
    "status",
    "xPos",
    "yPos",
    "zPos",
];

/// Containers list their palettes in a different order, and an empty heightmap
/// compound is left out on write, so these are compared as values.
const COMPARED_AS_VALUES: [&str; 3] = ["Heightmaps", "block_entities", "sections"];

#[test]
fn every_chunk_written_by_the_game_round_trips_over_what_the_shape_models() {
    for (name, bytes) in [
        ("chunk_full", FULL),
        ("chunk_terrain", TERRAIN),
        ("chunk_retrogen", RETROGEN),
        ("chunk_retrogen_minimal", RETROGEN_MINIMAL),
    ] {
        let original = load(bytes);
        let written = write_chunk(&original.chunk, &original.blocks, &original.biomes).unwrap();
        let read = load(&written);
        assert_same_chunk(&read, &original, name);

        let game = root(bytes);
        let mcrs = root(&written);
        for key in SAME_AS_THE_GAME {
            assert_eq!(
                game.get(key).map(sorted),
                mcrs.get(key).map(sorted),
                "{name}: {key}"
            );
        }

        let mut expected = vec!["PostProcessing", "block_ticks", "fluid_ticks", "structures"];
        if original.chunk.status != ChunkStatus::Full {
            expected.push("entities");
        }
        expected.sort_unstable();
        let left_out: Vec<&str> = keys(&game)
            .into_iter()
            .filter(|key| !SAME_AS_THE_GAME.contains(key) && !COMPARED_AS_VALUES.contains(key))
            .collect();
        assert_eq!(
            left_out, expected,
            "{name}: keys the game wrote that the shape leaves out"
        );
    }
}

#[test]
fn a_minimal_retrogen_is_written_back_with_the_keys_the_game_wrote() {
    let original = load(RETROGEN_MINIMAL);
    let written = write_chunk(&original.chunk, &original.blocks, &original.biomes).unwrap();
    let game = root(RETROGEN_MINIMAL);
    let mcrs = root(&written);
    let game_keys = keys(game.get_compound("retrogen").unwrap());
    assert_eq!(game_keys, ["statuses_to_rerun", "target_status"]);
    assert_eq!(keys(mcrs.get_compound("retrogen").unwrap()), game_keys);
}
