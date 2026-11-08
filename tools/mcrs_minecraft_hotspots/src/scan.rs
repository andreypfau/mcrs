use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use mcrs_minecraft_anvil::{PaletteLookup, Properties, RegionFile, parse_chunk};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_chunk::container::PalettedContainer;
use mcrs_voxel::ColumnPos;
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Part {
    RandomTick,
    /// Ticks randomly, but grows in natural terrain too, so it does not mark a farm.
    Terrain,
    Redstone,
}

/// A name that starts with `_` matches every block that ends with it.
pub const TRACKED: &[(&str, Part)] = &[
    ("wheat", Part::RandomTick),
    ("carrots", Part::RandomTick),
    ("potatoes", Part::RandomTick),
    ("beetroots", Part::RandomTick),
    ("torchflower_crop", Part::RandomTick),
    ("pitcher_crop", Part::RandomTick),
    ("pumpkin_stem", Part::RandomTick),
    ("melon_stem", Part::RandomTick),
    ("nether_wart", Part::RandomTick),
    ("sweet_berry_bush", Part::RandomTick),
    ("cocoa", Part::RandomTick),
    ("kelp", Part::RandomTick),
    ("sugar_cane", Part::RandomTick),
    ("bamboo", Part::RandomTick),
    ("bamboo_sapling", Part::RandomTick),
    ("cactus", Part::RandomTick),
    ("_sapling", Part::RandomTick),
    ("mangrove_propagule", Part::RandomTick),
    ("grass_block", Part::Terrain),
    ("mycelium", Part::Terrain),
    ("crimson_nylium", Part::Terrain),
    ("warped_nylium", Part::Terrain),
    ("_leaves", Part::Terrain),
    ("vine", Part::Terrain),
    ("twisting_vines", Part::Terrain),
    ("weeping_vines", Part::Terrain),
    ("cave_vines", Part::Terrain),
    ("farmland", Part::RandomTick),
    ("chorus_flower", Part::RandomTick),
    ("budding_amethyst", Part::RandomTick),
    ("pointed_dripstone", Part::Terrain),
    ("brown_mushroom", Part::RandomTick),
    ("red_mushroom", Part::RandomTick),
    ("open_eyeblossom", Part::RandomTick),
    ("closed_eyeblossom", Part::RandomTick),
    ("turtle_egg", Part::RandomTick),
    ("ice", Part::Terrain),
    ("snow", Part::Terrain),
    ("lava", Part::Terrain),
    ("redstone_wire", Part::Redstone),
    ("repeater", Part::Redstone),
    ("comparator", Part::Redstone),
    ("observer", Part::Redstone),
    ("piston", Part::Redstone),
    ("sticky_piston", Part::Redstone),
    ("hopper", Part::Redstone),
    ("dropper", Part::Redstone),
    ("dispenser", Part::Redstone),
    ("redstone_torch", Part::Redstone),
    ("redstone_wall_torch", Part::Redstone),
    ("redstone_lamp", Part::Redstone),
    ("rail", Part::Redstone),
    ("powered_rail", Part::Redstone),
    ("detector_rail", Part::Redstone),
    ("activator_rail", Part::Redstone),
];

pub struct ChunkLoad {
    pub entities: BTreeMap<String, u32>,
    pub block_entities: BTreeMap<String, u32>,
    pub block_ticks: BTreeMap<String, u32>,
    pub fluid_ticks: u32,
    /// The count of each block of `TRACKED`, after the count of all other blocks.
    pub tracked: Vec<u32>,
}

impl Default for ChunkLoad {
    fn default() -> Self {
        Self {
            entities: BTreeMap::new(),
            block_entities: BTreeMap::new(),
            block_ticks: BTreeMap::new(),
            fluid_ticks: 0,
            tracked: vec![0; TRACKED.len() + 1],
        }
    }
}

impl ChunkLoad {
    pub fn tracked(&self) -> impl Iterator<Item = (&'static str, Part, u32)> + '_ {
        TRACKED
            .iter()
            .zip(&self.tracked[1..])
            .map(|((name, part), count)| (*name, *part, *count))
    }

    pub fn parts(&self, part: Part) -> u32 {
        self.tracked()
            .filter(|(_, of, _)| *of == part)
            .map(|(_, _, count)| count)
            .sum()
    }
}

/// Maps a tracked block to its row of `TRACKED` plus one, and every other block to 0.
struct Tracked;

impl PaletteLookup<VoxelId> for Tracked {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<VoxelId> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        if name.ends_with("_leaves") && properties.get("persistent") != Some("false") {
            return Some(VoxelId(0));
        }
        let row = TRACKED.iter().position(|(tracked, _)| {
            name == *tracked || (tracked.starts_with('_') && name.ends_with(tracked))
        });
        Some(VoxelId(row.map_or(0, |row| row as u16 + 1)))
    }
}

struct AnyBiome;

impl PaletteLookup<u8> for AnyBiome {
    fn resolve(&self, _: &str, _: Properties<'_>) -> Option<u8> {
        Some(0)
    }
}

#[derive(Deserialize)]
struct Ticks {
    #[serde(default)]
    block_ticks: Vec<Tick>,
}

#[derive(Deserialize)]
struct Tick {
    i: String,
}

#[derive(Deserialize)]
struct Entities {
    #[serde(rename = "Entities", default)]
    entities: Vec<SavedEntity>,
}

#[derive(Deserialize)]
struct SavedEntity {
    id: String,
    #[serde(rename = "Passengers", default)]
    passengers: Vec<SavedEntity>,
}

fn count_entity(entity: &SavedEntity, into: &mut BTreeMap<String, u32>) {
    *into.entry(entity.id.clone()).or_default() += 1;
    for passenger in &entity.passengers {
        count_entity(passenger, into);
    }
}

/// The decompressed record of each present slot of the region files in `dir`.
fn records(dir: &Path) -> impl Iterator<Item = (ColumnPos, Option<Vec<u8>>)> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "mca"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| RegionFile::open(path).ok())
        .flat_map(|region| {
            let present: Vec<ColumnPos> = region.present().collect();
            present
                .into_iter()
                .map(move |pos| (pos, region.read_nbt(pos).ok().flatten()))
        })
}

/// The load of every saved column of the dimension at `dir`.
pub fn scan(dir: &Path) -> HashMap<ColumnPos, ChunkLoad> {
    let mut chunks: HashMap<ColumnPos, ChunkLoad> = HashMap::new();
    let mut failed = 0;
    for (pos, nbt) in records(&dir.join("region")) {
        let Some(chunk) = nbt
            .as_ref()
            .and_then(|nbt| parse_chunk(nbt, &Tracked, &AnyBiome).ok())
        else {
            failed += 1;
            continue;
        };
        let load = chunks.entry(pos).or_default();
        for section in &chunk.sections {
            match &section.block_states {
                Some(PalettedContainer::Homogeneous(id)) => {
                    load.tracked[usize::from(id.0)] += 4096;
                }
                Some(PalettedContainer::Heterogeneous(data)) => {
                    for (id, count) in data.palette.iter().zip(&data.counts) {
                        load.tracked[usize::from(id.0)] += u32::from(*count);
                    }
                }
                None => {}
            }
        }
        for block_entity in &chunk.block_entities {
            let id = block_entity.get_string("id").unwrap_or("?").to_owned();
            *load.block_entities.entry(id).or_default() += 1;
        }
        load.fluid_ticks = chunk.fluid_ticks.len() as u32;
        if let Some(Ok(ticks)) = nbt
            .as_ref()
            .map(|nbt| mcrs_minecraft_nbt::from_bytes::<Ticks>(Cursor::new(nbt)))
        {
            for tick in ticks.block_ticks {
                *load.block_ticks.entry(tick.i).or_default() += 1;
            }
        }
    }
    for (pos, nbt) in records(&dir.join("entities")) {
        let Some(Ok(file)) =
            nbt.map(|nbt| mcrs_minecraft_nbt::from_bytes::<Entities>(Cursor::new(nbt)))
        else {
            continue;
        };
        let load = chunks.entry(pos).or_default();
        for entity in &file.entities {
            count_entity(entity, &mut load.entities);
        }
    }
    eprintln!(
        "{}: {} columns, {failed} failed",
        dir.display(),
        chunks.len()
    );
    chunks
}
