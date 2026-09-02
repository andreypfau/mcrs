use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::{DATA_VERSION, LIGHT_BYTES};

pub const CHUNKS: usize = 1024;
pub const SECTIONS_PER_CHUNK: usize = 24;

pub fn region_chunks() -> Vec<Vec<u8>> {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    (0..CHUNKS)
        .map(|i| chunk(i as i32 % 32, i as i32 / 32, &mut rng))
        .collect()
}

pub fn cell_count() -> u64 {
    (CHUNKS * SECTIONS_PER_CHUNK * 4096) as u64
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn percent(&mut self, chance: usize) -> bool {
        self.below(100) < chance
    }
}

// Shares counted over `r.0.0.mca`; generating the region instead of shipping
// one keeps the bench on the current `DataVersion`.
fn block_palette_len(rng: &mut Rng) -> usize {
    match rng.below(1000) {
        0..=619 => 1,
        620..=626 => 3,
        627..=976 => 5 + rng.below(12),
        977..=996 => 17 + rng.below(16),
        _ => 33 + rng.below(32),
    }
}

fn chunk(x: i32, z: i32, rng: &mut Rng) -> Vec<u8> {
    let sections = (0..SECTIONS_PER_CHUNK)
        .map(|y| NbtTag::Compound(section(y as i8 - 4, rng)))
        .collect();

    let mut root = NbtCompound::new();
    root.put_int("DataVersion", DATA_VERSION);
    root.put_int("xPos", x);
    root.put_int("zPos", z);
    root.put_int("yPos", -4);
    root.put_string("Status", "minecraft:full".to_string());
    root.put_list("sections", sections);
    root.put_component("Heightmaps", heightmaps());
    root.put_bool("isLightOn", true);
    root.put_list("block_entities", block_entities(rng));
    root.put_long("InhabitedTime", 0);
    root.put_long("LastUpdate", 0);

    mcrs_minecraft_nbt::Nbt::new(String::new(), root)
        .write()
        .to_vec()
}

fn section(y: i8, rng: &mut Rng) -> NbtCompound {
    let mut s = NbtCompound::new();
    s.put_byte("Y", y);
    s.put_component(
        "block_states",
        container(block_palette_len(rng), 4096, 4, block_entry, rng),
    );
    s.put_component(
        "biomes",
        container(
            if rng.percent(78) { 1 } else { 2 + rng.below(3) },
            64,
            1,
            biome_entry,
            rng,
        ),
    );
    if rng.percent(13) {
        s.put("SkyLight", NbtTag::ByteArray(light(rng)));
        s.put("BlockLight", NbtTag::ByteArray(light(rng)));
    }
    s
}

fn container(
    len: usize,
    cells: usize,
    min_bits: u32,
    entry: fn(usize) -> NbtTag,
    rng: &mut Rng,
) -> NbtCompound {
    let mut c = NbtCompound::new();
    c.put_list("palette", (0..len).map(entry).collect());
    if len > 1 {
        let bits = mcrs_voxel_storage::ceillog2(len).max(min_bits);
        let indices: Vec<u16> = (0..cells).map(|_| rng.below(len) as u16).collect();
        c.put(
            "data",
            NbtTag::LongArray(
                mcrs_voxel_storage::pack_from(bits, &indices, |&i| i as u32).into_vec(),
            ),
        );
    }
    c
}

fn block_entry(index: usize) -> NbtTag {
    const NAMES: [&str; 8] = [
        "minecraft:air",
        "minecraft:stone",
        "minecraft:deepslate",
        "minecraft:water",
        "minecraft:dirt",
        "minecraft:grass_block",
        "minecraft:oak_log",
        "minecraft:andesite",
    ];
    let mut entry = NbtCompound::new();
    entry.put_string("id", NAMES[index % NAMES.len()].to_string());
    if index.is_multiple_of(3) {
        let mut props = NbtCompound::new();
        props.put_string("axis", ["x", "y", "z"][index % 3].to_string());
        props.put_string("waterlogged", "false".to_string());
        entry.put_component("properties", props);
    }
    NbtTag::Compound(entry)
}

fn biome_entry(index: usize) -> NbtTag {
    const NAMES: [&str; 4] = [
        "minecraft:plains",
        "minecraft:ocean",
        "minecraft:forest",
        "minecraft:the_void",
    ];
    NbtTag::String(NAMES[index % NAMES.len()].to_string())
}

fn light(rng: &mut Rng) -> Box<[u8]> {
    (0..LIGHT_BYTES).map(|_| rng.below(256) as u8).collect()
}

fn heightmaps() -> NbtCompound {
    let mut maps = NbtCompound::new();
    for name in ["WORLD_SURFACE", "MOTION_BLOCKING", "OCEAN_FLOOR"] {
        maps.put(name, NbtTag::LongArray(vec![0x0123_4567_89ab_cdef; 37]));
    }
    maps
}

fn block_entities(rng: &mut Rng) -> Vec<NbtTag> {
    (0..rng.below(2))
        .map(|_| {
            let mut be = NbtCompound::new();
            be.put_string("id", "minecraft:chest".to_string());
            be.put_int("x", rng.below(16) as i32);
            be.put_int("y", rng.below(256) as i32);
            be.put_int("z", rng.below(16) as i32);
            NbtTag::Compound(be)
        })
        .collect()
}
