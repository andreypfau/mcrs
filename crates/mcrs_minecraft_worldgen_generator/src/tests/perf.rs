//! What the ladder costs, measured rather than argued: the three stages of a
//! column, the terrain descent inside the first of them, the live centre map
//! updates inside the second, and what one staged snapshot weighs.
//!
//! ```text
//! cargo test --release -p mcrs_minecraft_worldgen_generator the_ladder_costs -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use mcrs_minecraft_chunk::PalettedContainer;

use crate::ColumnBlocks;
use crate::heightmap::{HeightmapPredicates, build_terrain_heightmaps};
use crate::stages::{fill_column, merge_column, run_region};
use crate::staging::{FilledSnapshot, Stage, StagingStore, rank};
use crate::task::CancellationToken;

/// The widest consumer there is: the forest tree feature, whose crowns and
/// decorators reach past the column that seeds them and so read the ring.
const BIOME: &str = "minecraft:forest";
const FEATURE: &str = "minecraft:trees_birch_and_oak_leaf_litter";
const SEED: u64 = 4242;

/// Columns merged; runs cover one more ring, fills two.
const RADIUS: i32 = 4;
/// Discarded before anything is recorded, per `PERF.md`.
const WARMUP: i32 = 2;

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// The heap one staged snapshot owns: its sections and its six maps.
fn snapshot_bytes(snapshot: &FilledSnapshot) -> usize {
    let mut bytes = 0;
    for section in snapshot.sections.iter().flatten() {
        let (blocks, biomes) = section;
        if let PalettedContainer::Heterogeneous(data) = &blocks.0 {
            bytes += 4096 * 2 + data.palette.len() * 4;
        }
        if let PalettedContainer::Heterogeneous(data) = &biomes.0 {
            bytes += 64 + data.palette.len() * 3;
        }
    }
    // 256 heights of nine bits, six generations.
    bytes + 6 * 288
}

use super::ladder::region_columns as square;

#[test]
#[ignore = "a measurement, not an assertion"]
fn the_ladder_costs() {
    let (ctx, y_sections) = super::trees::tree_dimension(BIOME, FEATURE, SEED);
    let predicates = ctx.predicates.clone().expect("the dimension has the table");
    let cancel = CancellationToken::new();
    let mut buffer = ColumnBlocks::new(&y_sections);
    let mut store = StagingStore::default();

    let mut fills = Vec::new();
    let mut descents = Vec::new();
    let mut unpacks = Vec::new();
    let mut bytes = Vec::new();
    for col in square(RADIUS + 2) {
        let started = Instant::now();
        let Some(snapshot) = fill_column(&ctx, col, &mut buffer, &cancel) else {
            continue;
        };
        let fill = started.elapsed();

        let started = Instant::now();
        let dense = ColumnBlocks::from_sections(&snapshot.sections, &snapshot.y_sections);
        let unpack = started.elapsed();
        let started = Instant::now();
        let descent = build_terrain_heightmaps(&dense, &predicates);
        let took = started.elapsed();
        assert!(descent.is_some(), "the descent answered");

        if col.x.abs() > WARMUP || col.z.abs() > WARMUP {
            fills.push(fill);
            descents.push(took);
            unpacks.push(unpack);
            bytes.push(snapshot_bytes(&snapshot));
        }
        store.insert_filled(snapshot);
        store.set_stage(col, Stage::Filled);
    }

    // Every rung of the ladder, so a column's run and merge samples are what it
    // costs to decorate it whole rather than what one rung of it costs.
    let mut runs = Vec::new();
    let mut merges = Vec::new();
    for rung in 0..ctx.rungs() {
        for col in square(RADIUS + 1) {
            let Some(region) = store.region(col, rung) else {
                continue;
            };
            let started = Instant::now();
            let deltas = run_region(&ctx, &region, rung);
            let took = started.elapsed();
            if col.x.abs() > WARMUP || col.z.abs() > WARMUP {
                runs.push(took);
            }
            for (target, delta) in deltas {
                store.push_delta(col, rung, target, delta);
            }
            store.set_stage(col, Stage::Ran(rung as u8));
        }
        for col in square(RADIUS) {
            let deltas = store.deltas(col, rung);
            let Some(snapshot) = store.base(col, rung).cloned() else {
                continue;
            };
            let started = Instant::now();
            let merged = merge_column(
                &snapshot,
                &deltas,
                Some(&predicates),
                ctx.features()
                    .map(|program| &program.world.has_block_entity),
            );
            let took = started.elapsed();
            assert_eq!(merged.sections.len(), snapshot.sections.len());
            if col.x.abs() > WARMUP || col.z.abs() > WARMUP {
                merges.push(took);
            }
            store.insert_staged(col, rung, std::sync::Arc::new(merged));
            store.set_stage(col, Stage::Merged(rung as u8));
        }
    }

    let fill = median(fills);
    let run = median(runs.clone());
    let (live_maps, map_updates) = replay_live_maps(&store, &predicates);
    let live_maps = median(live_maps);
    let mean_bytes = bytes.iter().sum::<usize>() / bytes.len();
    println!(
        "biome {BIOME}, feature {FEATURE}, seed {SEED}, {} columns",
        runs.len()
    );
    println!("Filled        {:.3} ms", ms(fill));
    println!("  terrain     {:.3} ms", ms(median(descents)));
    println!("Run           {:.3} ms", ms(run));
    println!(
        "  live maps   {:.3} ms over {map_updates} updates",
        ms(live_maps)
    );
    println!("Merged        {:.3} ms", ms(median(merges)));
    println!(
        "unpack        {:.3} ms per dense buffer",
        ms(median(unpacks))
    );
    println!(
        "snapshot      {} KB, {} sections",
        mean_bytes / 1024,
        y_sections.len()
    );
    // `cancel_stale_columns` keeps every column within two of a wanted one, and
    // a merged column outlives its delivery so a late section is delivered from
    // the same blocks: the store holds both squares at once.
    for distance in [8u32, 13, 32, 96] {
        let filled = (2 * (distance as usize + 2) + 1).pow(2);
        let merged = (2 * distance as usize + 1).pow(2);
        println!(
            "store at view {distance}: {filled} filled + {merged} merged, {} MB",
            (filled + merged) * mean_bytes / (1024 * 1024)
        );
    }
}

/// The live centre map updates of one run, timed on their own.
///
/// The two cannot be separated by running with the maps switched off: the four
/// final maps are read back within the run, so a run without them places
/// different blocks. What is replayed instead is the write list the run
/// produced, against the same column and the same starting maps.
fn replay_live_maps(
    store: &StagingStore,
    predicates: &HeightmapPredicates,
) -> (Vec<Duration>, usize) {
    let mut samples = Vec::new();
    let mut updates = 0;
    for col in square(RADIUS) {
        let Some(snapshot) = store.filled(col) else {
            continue;
        };
        let (Some(start), Some(own)) = (
            snapshot.maps.clone(),
            store
                .deltas(col, 0)
                .into_iter()
                .find(|delta| delta.source_rank == rank(col)),
        ) else {
            continue;
        };
        let column = ColumnBlocks::from_sections(&snapshot.sections, &snapshot.y_sections);
        for (cell, state) in &own.writes {
            let slot = *cell as usize / ColumnBlocks::SECTION_VOLUME;
            let within = *cell as usize % ColumnBlocks::SECTION_VOLUME;
            column.section_cells(slot)[within].set(*state);
        }

        let mut maps = start;
        let started = Instant::now();
        for (cell, state) in &own.writes {
            let slot = *cell as usize / ColumnBlocks::SECTION_VOLUME;
            let within = *cell as usize % ColumnBlocks::SECTION_VOLUME;
            let (x, z) = (within & 15, (within >> 4) & 15);
            let y = snapshot.y_sections[slot] * 16 + (within >> 8) as i32;
            let read = |at: i32| column.get(x as i32, at, z as i32).unwrap_or_default();
            maps.apply_write(x, z, y, predicates.get(*state), predicates, &read);
        }
        samples.push(started.elapsed());
        updates += own.writes.len();
    }
    let per_run = updates / samples.len().max(1);
    (samples, per_run)
}
