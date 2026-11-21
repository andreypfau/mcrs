#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a benchmark has no caller to return an error to"
)]

mod common;
use mcrs_minecraft_worldgen_density::aquifer_cells::AquiferCells;
use mcrs_minecraft_worldgen_generator::{ClimateBox, ColumnBiomes, cover_fill_group};
use mcrs_voxel::ColumnPos;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mcrs_minecraft_biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_biome::parameter_list::Preset;
use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_value_provider::HeightContext;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_generator::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, TerrainCarving, modern_carving_mask,
};
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::MultiNoiseBiomeTable;
use mcrs_minecraft_worldgen_generator::task::CancellationToken;
use mcrs_minecraft_worldgen_generator::{
    ColumnBlocks, NO_TOP, SurfaceIds, SurfaceStates, apply_material_surface, fill_column_dense_any,
    multi_noise_grid, multi_noise_palettes,
};
use mcrs_minecraft_worldgen_surface::MaterialScratch;

use common::support::{biome_id, biomes_of, carvers_of, corpus, material_router, registries_over};

static CHECK: AtomicU64 = AtomicU64::new(0);
static BIOME_CHECK: AtomicU64 = AtomicU64::new(0);
const ABSENT_BIOME: u16 = 250;
const GROUP: usize = 4;

fn biome_ids() -> HashMap<String, u16> {
    let mut ids = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = u16::try_from(ids.len()).unwrap();
        ids.entry(biome.to_string()).or_insert(next);
    }
    ids
}

fn column_hash(column: &ColumnBlocks) -> u64 {
    let mut hash = 0u64;
    for y in -64..320 {
        for z in 0..16 {
            for x in 0..16 {
                let state = column
                    .get(x, y, z)
                    .map_or(u64::MAX, |state| u64::from(u16::from(state)));
                hash = (hash ^ state).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    hash
}

fn biome_hash(sections: &[Option<(BlockPalette, BiomePalette)>]) -> u64 {
    let mut hash = 0u64;
    let mut mix = |value: u64| hash = (hash ^ value).wrapping_mul(0x0100_0000_01b3);
    for section in sections {
        let Some((_, biomes)) = section else {
            mix(u64::MAX);
            continue;
        };
        biomes.for_each_distinct(|biome| mix(u64::from(biome)));
        mix(u64::MAX - 1);
        for y in 0..BiomePalette::SIZE {
            for z in 0..BiomePalette::SIZE {
                for x in 0..BiomePalette::SIZE {
                    mix(biomes.get_cell(x, y, z).map_or(u64::MAX, u64::from));
                }
            }
        }
    }
    hash
}

#[cfg(unix)]
const THREAD_CLOCK: &str = "thread cpu time";

#[cfg(not(unix))]
const THREAD_CLOCK: &str = "wall time";

#[cfg(unix)]
fn thread_time() -> Duration {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `now` is a valid, exclusively borrowed timespec for the whole call.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &raw mut now) };
    assert_eq!(status, 0, "the thread cpu clock is available");
    Duration::new(now.tv_sec.unsigned_abs(), now.tv_nsec.unsigned_abs() as u32)
}

#[cfg(not(unix))]
fn thread_time() -> Duration {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
}

#[derive(Default, Clone, Copy)]
struct Stages {
    fill: Duration,
    biomes: Duration,
    surface: Duration,
    carve: Duration,
    pack: Duration,
}

#[expect(
    clippy::too_many_lines,
    reason = "reads the arguments, builds the router and material rules, then times each stage over the same region and prints the table of costs"
)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let side: i32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(8);
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(845);
    let offset: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let repeats: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1).max(1);

    let ids = biome_ids();
    let mut names = vec![String::from("?"); ids.len().max(ABSENT_BIOME as usize + 1)];
    for (name, &id) in &ids {
        names[id as usize] = name.strip_prefix("minecraft:").unwrap_or(name).to_owned();
    }
    let mut numbered: Vec<(&u16, &str)> =
        ids.iter().map(|(name, id)| (id, name.as_str())).collect();
    numbered.sort();
    let leading: Vec<&str> = numbered.into_iter().map(|(_, name)| name).collect();
    let registries = registries_over(&leading);
    let (router, material) = material_router("overworld", seed, &registries);
    let table = MultiNoiseBiomeTable::of_preset(Preset::Overworld, biomes_of(&registries)).unwrap();
    let carvers = CarverBiomeTable::from_climate(Preset::Overworld.parameter_list(), carvers_of);
    let carver_ids = ModernCarverBlockIds::resolve(corpus(), []);
    let biome = |name: &str| biome_id(&registries, name);
    let surface_ids = SurfaceIds {
        eroded_badlands: biome("minecraft:eroded_badlands"),
        frozen_ocean: biome("minecraft:frozen_ocean"),
        deep_frozen_ocean: biome("minecraft:deep_frozen_ocean"),
    };
    let surface_states = SurfaceStates::new(corpus());
    let height = HeightContext {
        min_y: router.noise.min_y,
        depth: router.noise.height as i32,
        sea_level: router.sea_level,
    };
    let y_sections: Vec<i32> = (-4..20).collect();
    let cancel = CancellationToken::new();

    let mut column = ColumnBlocks::new(&y_sections);
    let mut scratch = MaterialScratch::default();
    let mut ws = Workspace::new();
    let mut cells = AquiferCells::default();
    let mut sink = 0u64;

    let run = |column: &mut ColumnBlocks,
               scratch: &mut MaterialScratch,
               ws: &mut Workspace,
               climate: &ClimateBox,
               cells: &mut AquiferCells,
               x: i32,
               z: i32|
     -> (Stages, u64, u8) {
        let mut s = Stages::default();
        let t = thread_time();
        let mut filled = fill_column_dense_any(
            column,
            x,
            z,
            &y_sections,
            &router,
            cells,
            &ColumnBiomes::None,
            None,
            None,
            &cancel,
        )
        .unwrap();
        s.fill = thread_time() - t;
        let t = thread_time();
        let biomes =
            multi_noise_palettes(&router, &table, Some(climate), x * 16, z * 16, &y_sections);
        s.biomes = thread_time() - t;

        let dominant = {
            let g = multi_noise_grid(&router, &table, Some(climate), x * 16, z * 16, &y_sections)
                .unwrap();
            let mut counts = [0u32; 256];
            for cz in 0..4i32 {
                for cx in 0..4i32 {
                    let top = filled.tops[((cz * 4 + 1) * 16 + cx * 4 + 1) as usize];
                    let block_y = if top == NO_TOP {
                        g.volume.min_block().y
                    } else {
                        top
                    };
                    let cy = (block_y - g.volume.min_block().y)
                        .div_euclid(4)
                        .clamp(0, g.volume.size().y - 1);
                    counts[g.get(cx + 1, cy, cz + 1) as usize] += 1;
                }
            }
            counts
                .iter()
                .enumerate()
                .max_by_key(|&(_, &c)| c)
                .map(|(id, _)| id as u8)
                .unwrap()
        };

        let t = thread_time();
        let mask = modern_carving_mask(x, z, seed as i64, &router, ws, &carvers, height);
        s.carve = thread_time() - t;

        let t = thread_time();
        apply_material_surface(
            column,
            x,
            z,
            &mut filled.tops,
            &biomes,
            y_sections[0],
            &router,
            &material,
            &surface_ids,
            &surface_states,
            scratch,
            Some(&mut TerrainCarving::new(
                &mask,
                &carver_ids,
                &mut filled.fluid,
                &router,
                x,
                z,
            )),
        );
        s.surface = thread_time() - t;
        CHECK.fetch_xor(
            column_hash(column).rotate_left((x * 7 + z * 13) as u32 & 63),
            Ordering::Relaxed,
        );

        let t = thread_time();
        let sections = column.into_sections(&biomes);
        s.pack = thread_time() - t;
        BIOME_CHECK.fetch_xor(
            biome_hash(&sections).rotate_left((x * 7 + z * 13) as u32 & 63),
            Ordering::Relaxed,
        );
        (s, sections.len() as u64, dominant)
    };

    let warm: Vec<ColumnPos> = (0..2)
        .map(|x| ColumnPos::new(offset + x, offset - 1))
        .collect();
    let climate = ClimateBox::sample(&router, &table, warm.iter().copied(), &y_sections).unwrap();
    cover_fill_group(&router, warm.iter().copied(), &mut cells);
    for col in &warm {
        run(
            &mut column,
            &mut scratch,
            &mut ws,
            &climate,
            &mut cells,
            col.x,
            col.z,
        );
    }

    let warm_check = CHECK.load(Ordering::Relaxed);
    let warm_biome_check = BIOME_CHECK.load(Ordering::Relaxed);
    let mut per_column: Vec<f64> = Vec::new();
    let mut per_biome: BTreeMap<u8, (u32, Stages)> = BTreeMap::new();
    let mut best_all = Duration::MAX;
    let mut wall = Duration::ZERO;
    let mut lowest: Option<Stages> = None;
    for _ in 0..repeats {
        CHECK.store(warm_check, Ordering::Relaxed);
        BIOME_CHECK.store(warm_biome_check, Ordering::Relaxed);
        sink = 0;
        let mut total = Stages::default();
        let mut round_columns: Vec<f64> = Vec::with_capacity((side * side) as usize);
        let mut round_biome: BTreeMap<u8, (u32, Stages)> = BTreeMap::new();
        let started = Instant::now();
        for group_z in (0..side).step_by(GROUP) {
            for group_x in (0..side).step_by(GROUP) {
                let group: Vec<ColumnPos> = (group_z..side.min(group_z + GROUP as i32))
                    .flat_map(|z| {
                        (group_x..side.min(group_x + GROUP as i32))
                            .map(move |x| ColumnPos::new(offset + x, offset + z))
                    })
                    .collect();
                let t = thread_time();
                let climate =
                    ClimateBox::sample(&router, &table, group.iter().copied(), &y_sections)
                        .unwrap();
                let share = (thread_time() - t) / group.len() as u32;
                let t = thread_time();
                cover_fill_group(&router, group.iter().copied(), &mut cells);
                let cover_share = (thread_time() - t) / group.len() as u32;
                for col in group {
                    let (mut s, n, dominant) = run(
                        &mut column,
                        &mut scratch,
                        &mut ws,
                        &climate,
                        &mut cells,
                        col.x,
                        col.z,
                    );
                    s.biomes += share;
                    s.fill += cover_share;
                    total.fill += s.fill;
                    total.biomes += s.biomes;
                    total.surface += s.surface;
                    total.carve += s.carve;
                    total.pack += s.pack;
                    sink += n;

                    let one = s.fill + s.biomes + s.surface + s.carve + s.pack;
                    round_columns.push(one.as_secs_f64() * 1e3);
                    let slot = round_biome.entry(dominant).or_default();
                    slot.0 += 1;
                    slot.1.fill += s.fill;
                    slot.1.biomes += s.biomes;
                    slot.1.surface += s.surface;
                    slot.1.carve += s.carve;
                    slot.1.pack += s.pack;
                }
            }
        }
        let round_wall = started.elapsed();
        let round_all = total.fill + total.biomes + total.surface + total.carve + total.pack;
        if round_all < best_all {
            best_all = round_all;
            wall = round_wall;
            per_column = round_columns;
            per_biome = round_biome;
        }
        lowest = Some(lowest.map_or(total, |low| Stages {
            fill: low.fill.min(total.fill),
            biomes: low.biomes.min(total.biomes),
            surface: low.surface.min(total.surface),
            carve: low.carve.min(total.carve),
            pack: low.pack.min(total.pack),
        }));
    }
    let total = lowest.unwrap_or_default();
    let n = (side * side) as f64;
    let ms = |d: Duration| d.as_secs_f64() * 1e3 / n;
    let all = total.fill + total.biomes + total.surface + total.carve + total.pack;
    let pct = |d: Duration| 100.0 * d.as_secs_f64() / all.as_secs_f64();
    println!("times are {THREAD_CLOCK}, minimum over {repeats} repeat(s) per stage");
    println!(
        "seed={seed} columns={} at [{offset}..{}) sections={sink} wall={:.1} ms",
        side * side,
        offset + side,
        wall.as_secs_f64() * 1e3
    );
    println!("per column: {:.3} ms", ms(all));
    println!(
        "  fill    {:.3} ms  {:4.1}%",
        ms(total.fill),
        pct(total.fill)
    );
    println!(
        "  biomes  {:.3} ms  {:4.1}%",
        ms(total.biomes),
        pct(total.biomes)
    );
    println!(
        "  surface {:.3} ms  {:4.1}%",
        ms(total.surface),
        pct(total.surface)
    );
    println!(
        "  carve   {:.3} ms  {:4.1}%",
        ms(total.carve),
        pct(total.carve)
    );
    println!(
        "  pack    {:.3} ms  {:4.1}%",
        ms(total.pack),
        pct(total.pack)
    );

    println!("CHECK {:016x}", CHECK.load(Ordering::Relaxed));
    println!("BIOME_CHECK {:016x}", BIOME_CHECK.load(Ordering::Relaxed));
    per_column.sort_by(f64::total_cmp);
    let at =
        |q: f64| per_column[(((per_column.len() as f64) * q) as usize).min(per_column.len() - 1)];
    println!(
        "spread: p50 {:.3} ms  p90 {:.3} ms  max {:.3} ms",
        at(0.5),
        at(0.9),
        per_column[per_column.len() - 1]
    );

    let mut rows: Vec<_> = per_biome.into_iter().collect();
    rows.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    println!("by dominant biome:      cols     all    fill  biomes surface");
    for (id, (n, s)) in rows {
        let each = |d: Duration| d.as_secs_f64() * 1e3 / n as f64;
        let all = s.fill + s.biomes + s.surface + s.carve + s.pack;
        println!(
            "  {:<22} {:4}  {:6.3}  {:6.3}  {:6.3}  {:6.3}",
            names.get(id as usize).map_or("?", String::as_str),
            n,
            each(all),
            each(s.fill),
            each(s.biomes),
            each(s.surface),
        );
    }
}
