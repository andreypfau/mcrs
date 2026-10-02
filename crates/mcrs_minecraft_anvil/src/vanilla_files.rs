use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ColumnPos;

use crate::{Chunk, ChunkStatus, PaletteNames, parse_chunk};

const FULL: &[u8] = include_bytes!("fixtures/vanilla/chunk_full.nbt");

struct Loaded {
    chunk: Chunk,
    blocks: PaletteNames<VoxelId>,
    biomes: PaletteNames<u8>,
}

fn load(bytes: &[u8]) -> Loaded {
    let (blocks, biomes) = (PaletteNames::new(), PaletteNames::new());
    let chunk = parse_chunk(bytes, &blocks, &biomes).unwrap();
    Loaded {
        chunk,
        blocks,
        biomes,
    }
}

fn biome(loaded: &Loaded, name: &str) -> u8 {
    loaded.biomes.intern(name, std::iter::empty()).unwrap()
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

    let ground = chunk.sections.iter().find(|s| s.y == 0).unwrap();
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

    let layered = chunk.sections.iter().find(|s| s.y == 1).unwrap();
    let biomes = layered.biomes.as_ref().unwrap();
    for (y, expected) in [
        (0, plains),
        (4, plains),
        (5, forest),
        (10, forest),
        (11, desert),
        (15, desert),
    ] {
        assert_eq!(biomes.get(7, y, 2), expected, "layer {y}");
    }
}
