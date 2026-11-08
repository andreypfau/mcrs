//! Ranks the farms and machines of a save by the server load they cause.
//!
//! `mcrs_minecraft_hotspots <world>` reads every column and entity file of the three vanilla
//! dimensions, joins neighbouring hot columns into sites and prints the top sites for each load.
//! Each further `<dimension> <x> <z>` triple prints the load within the simulation distance of
//! that block.
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a developer tool reports on stdout and panics on a broken invariant"
)]

mod scan;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use mcrs_voxel::ColumnPos;
use scan::{ChunkLoad, Part};

const DIMENSIONS: [&str; 3] = ["overworld", "the_nether", "the_end"];
const TOP: usize = 15;
/// Hot columns closer than this many columns form one site.
const GAP: i32 = 2;
/// The default `simulation-distance` of a dedicated server, in columns.
const SIMULATION: i32 = 10;

/// Entities that never move, so they add no entity physics.
const STILL: [&str; 5] = [
    "minecraft:item_frame",
    "minecraft:glow_item_frame",
    "minecraft:painting",
    "minecraft:leash_knot",
    "minecraft:marker",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Load {
    Entities,
    RandomTicks,
    Scheduled,
    BlockEntities,
}

const LOADS: [Load; 4] = [
    Load::Entities,
    Load::RandomTicks,
    Load::Scheduled,
    Load::BlockEntities,
];

impl Load {
    fn name(self) -> &'static str {
        match self {
            Self::Entities => "entity physics",
            Self::RandomTicks => "random ticks",
            Self::Scheduled => "scheduled ticks and redstone",
            Self::BlockEntities => "block entities",
        }
    }

    fn of(self, chunk: &ChunkLoad) -> u32 {
        match self {
            Self::Entities => chunk
                .entities
                .iter()
                .filter(|(id, _)| !STILL.contains(&id.as_str()))
                .map(|(_, count)| count)
                .sum(),
            Self::RandomTicks => chunk.parts(Part::RandomTick),
            Self::Scheduled => {
                chunk.block_ticks.values().sum::<u32>()
                    + chunk.fluid_ticks
                    + chunk.parts(Part::Redstone)
            }
            Self::BlockEntities => chunk.block_entities.values().sum(),
        }
    }

    /// The value from which a column counts as hot: well above a plain built-up column.
    fn hot(self) -> u32 {
        match self {
            Self::Entities => 24,
            Self::RandomTicks => 1024,
            Self::Scheduled => 96,
            Self::BlockEntities => 48,
        }
    }
}

struct Site<'d> {
    dimension: &'d str,
    columns: Vec<ColumnPos>,
    total: ChunkLoad,
}

impl<'d> Site<'d> {
    fn new(
        dimension: &'d str,
        columns: Vec<ColumnPos>,
        chunks: &HashMap<ColumnPos, ChunkLoad>,
    ) -> Self {
        let mut total = ChunkLoad::default();
        for chunk in columns.iter().filter_map(|pos| chunks.get(pos)) {
            merge(&mut total.entities, &chunk.entities);
            merge(&mut total.block_entities, &chunk.block_entities);
            merge(&mut total.block_ticks, &chunk.block_ticks);
            total.fluid_ticks += chunk.fluid_ticks;
            for (sum, count) in total.tracked.iter_mut().zip(&chunk.tracked) {
                *sum += count;
            }
        }
        Self {
            dimension,
            columns,
            total,
        }
    }
}

fn merge(into: &mut BTreeMap<String, u32>, from: &BTreeMap<String, u32>) {
    for (key, count) in from {
        *into.entry(key.clone()).or_default() += count;
    }
}

fn sites<'d>(
    dimension: &'d str,
    chunks: &HashMap<ColumnPos, ChunkLoad>,
    hot: impl Fn(&ChunkLoad) -> bool,
) -> Vec<Site<'d>> {
    let mut left: HashSet<ColumnPos> = chunks
        .iter()
        .filter(|(_, chunk)| hot(chunk))
        .map(|(pos, _)| *pos)
        .collect();
    let mut found = Vec::new();
    while let Some(&seed) = left.iter().next() {
        left.remove(&seed);
        let mut columns = vec![seed];
        let mut next = 0;
        while let Some(&at) = columns.get(next) {
            next += 1;
            for near in at.square(GAP) {
                if left.remove(&near) {
                    columns.push(near);
                }
            }
        }
        found.push(Site::new(dimension, columns, chunks));
    }
    found
}

fn top<'a>(counts: impl IntoIterator<Item = (&'a str, u32)>) -> String {
    let mut counts: Vec<(&str, u32)> = counts.into_iter().filter(|(_, n)| *n > 0).collect();
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counts
        .iter()
        .take(4)
        .map(|(name, count)| format!("{} {count}", name.trim_start_matches("minecraft:")))
        .collect::<Vec<_>>()
        .join(", ")
}

fn print(site: &Site<'_>) {
    let xs = site.columns.iter().map(|pos| pos.x);
    let zs = site.columns.iter().map(|pos| pos.z);
    let (x0, x1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0));
    let (z0, z1) = (zs.clone().min().unwrap_or(0), zs.max().unwrap_or(0));
    let total = &site.total;
    let items = total.entities.get("minecraft:item").copied().unwrap_or(0);
    println!(
        "  {} centre {} {} | blocks x {}..{} z {}..{} | {} columns | moving entities {} (items {}) random {} (terrain {}) scheduled {} (ticks {}, redstone {}) block entities {}",
        site.dimension,
        (x0 + x1) * 8 + 8,
        (z0 + z1) * 8 + 8,
        x0 * 16,
        x1 * 16 + 15,
        z0 * 16,
        z1 * 16 + 15,
        site.columns.len(),
        Load::Entities.of(&site.total),
        items,
        Load::RandomTicks.of(&site.total),
        total.parts(Part::Terrain),
        Load::Scheduled.of(&site.total),
        total.block_ticks.values().sum::<u32>() + total.fluid_ticks,
        total.parts(Part::Redstone),
        Load::BlockEntities.of(&site.total),
    );
    let named = |counts: &'_ BTreeMap<String, u32>| -> String {
        top(counts.iter().map(|(name, count)| (name.as_str(), *count)))
    };
    let tracked = total
        .tracked()
        .filter(|(_, part, _)| *part != Part::Terrain)
        .map(|(name, _, count)| (name, count));
    println!("    entities: {}", named(&total.entities));
    println!("    block entities: {}", named(&total.block_entities));
    println!("    block ticks: {}", named(&total.block_ticks));
    println!("    blocks: {}", top(tracked));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (world, centres) = args
        .split_first()
        .expect("usage: mcrs_minecraft_hotspots <world> [<dimension> <x> <z>]...");
    let world = PathBuf::from(world);
    let scans: Vec<(&str, HashMap<ColumnPos, ChunkLoad>)> = DIMENSIONS
        .iter()
        .map(|name| {
            (
                *name,
                scan::scan(&world.join("dimensions/minecraft").join(name)),
            )
        })
        .collect();
    let mut best = Vec::new();
    for load in LOADS {
        let mut ranked: Vec<Site<'_>> = scans
            .iter()
            .flat_map(|(name, chunks)| sites(name, chunks, |chunk| load.of(chunk) >= load.hot()))
            .collect();
        ranked.sort_by_key(|site| std::cmp::Reverse(load.of(&site.total)));
        println!("\n# {} (column hot from {})", load.name(), load.hot());
        for site in ranked.iter().take(TOP) {
            print(site);
        }
        best.push(ranked.first().map_or(1, |site| load.of(&site.total)).max(1));
    }
    let mut overall: Vec<(f64, Site<'_>)> = scans
        .iter()
        .flat_map(|(name, chunks)| {
            sites(name, chunks, |chunk| {
                LOADS.iter().any(|load| load.of(chunk) >= load.hot())
            })
        })
        .map(|site| {
            let score = LOADS
                .iter()
                .zip(&best)
                .map(|(load, best)| f64::from(load.of(&site.total)) / f64::from(*best))
                .sum();
            (score, site)
        })
        .collect();
    overall.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("\n# overall (sum of each load over its best site)");
    for (score, site) in overall.iter().take(TOP) {
        println!("  score {score:.2}");
        print(site);
    }
    println!("\n# within {SIMULATION} columns of a centre");
    for [dimension, x, z] in centres.as_chunks::<3>().0 {
        let column = |block: &str| block.parse::<i32>().expect("a block coordinate") >> 4;
        let (x, z) = (column(x), column(z));
        let (name, chunks) = scans
            .iter()
            .find(|(name, _)| name == dimension)
            .expect("a vanilla dimension");
        let columns = ColumnPos::new(x, z).square(SIMULATION).collect();
        print(&Site::new(name, columns, chunks));
    }
}
