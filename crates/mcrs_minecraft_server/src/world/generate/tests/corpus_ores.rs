//! The corpus's own ore features, compiled the way the freeze compiles them,
//! and the band their veins land in.

use mcrs_minecraft_core::BlockPos;
use std::collections::BTreeMap;
use std::sync::Arc;

use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_decoration::feature::ore_modern::{OreScratch, place_modern_ore};
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_worldgen::feature::compile::CompiledPlacedFeature;
use mcrs_minecraft_worldgen::feature::placer::{PlacerScratch, decorate};
use mcrs_minecraft_worldgen::feature::proto::{Feature, Holder};
use mcrs_voxel_storage::VoxelId;

use crate::world::generate::ColumnBlocks;
use crate::world::generate::feature_program::{FeatureProgram, Generator};
use crate::world::generate::features::FeatureTables;
use crate::world::generate::stages::{
    ColumnRegion, FillContext, decoration_seed, dimension_y_sections,
};
use crate::world::generate::staging::{FilledSnapshot, RegionSnapshots, region_column};

use super::{
    biome_registry, blocks, build_program, build_settings_router, corpus_features,
    fill_context_with, flat_snapshot, one_step,
};

/// The one biome of these tables, so that every ore is carried and the `biome`
/// placement filter passes wherever the palette says 0. A snapshot numbers its
/// entries in name order, so this one has to sort before the three below.
const BIOME: &str = "minecraft:badlands";

/// Every placed feature of the corpus whose feature is an `ore`, in id order,
/// and the name of the feature behind each — several placed features share one,
/// and one is what a count is per.
///
/// The order is this test's, not the sort's: a band over counts does not depend
/// on which order the seeds are drawn in, only on it being fixed.
pub(super) fn ore_tables() -> (FeatureTables, Vec<String>) {
    let corpus = corpus_features();

    let mut named: Vec<String> = Vec::new();
    let mut step: Vec<Arc<CompiledPlacedFeature>> = Vec::new();
    for (id, entry) in &corpus.placed_features {
        let feature = match &entry.feature {
            Holder::Reference(name) => corpus
                .features
                .get(name)
                .unwrap_or_else(|| panic!("{id} names {name}, which the corpus does not declare")),
            Holder::Inline(inline) => inline.as_ref(),
        };
        if !matches!(feature, Feature::Ore { .. }) {
            continue;
        }
        named.push(match &entry.feature {
            Holder::Reference(name) => name.as_str().to_owned(),
            Holder::Inline(_) => format!("{id} (inline)"),
        });
        step.push(Arc::new(CompiledPlacedFeature {
            id: Some(id.clone()),
            placed: entry.clone(),
        }));
    }
    (one_step(step, BIOME), named)
}

/// The one biome the tables carry, at id 0, and the three the surface stage
/// resolves by name off whatever registry the dimension holds — without them
/// `SurfaceIds::resolve` panics before a column is ever filled.
pub(super) fn one_biome_registry() -> RegistrySnapshot<Biome> {
    biome_registry(&[
        BIOME,
        "minecraft:eroded_badlands",
        "minecraft:frozen_ocean",
        "minecraft:deep_frozen_ocean",
    ])
}

pub(super) fn ore_program(tables: &FeatureTables) -> FeatureProgram {
    let program = build_program(tables, corpus_features(), &one_biome_registry(), 0);
    assert!(
        program.slot_of(0).is_some(),
        "{BIOME} is not the entry a snapshot numbers 0, so the palette these \
         tests fill with carries no features at all"
    );
    program
}

/// Which rock a band is measured on. The corpus's ore targets are two disjoint
/// families — the overworld's stone and deepslate replaceables, and the
/// nether's netherrack — so a single column of one rock would leave half the
/// features placing nothing.
#[derive(Clone, Copy, Debug)]
enum Terrain {
    Overworld,
    Nether,
}

impl Terrain {
    fn settings(self) -> &'static str {
        match self {
            Terrain::Overworld => "overworld",
            Terrain::Nether => "nether",
        }
    }

    /// The block a whole section is made of, or `None` for air.
    fn section(self, section_y: i32) -> Option<&'static str> {
        match self {
            Terrain::Overworld if section_y < 0 => Some("minecraft:deepslate"),
            Terrain::Overworld if section_y < 12 => Some("minecraft:stone"),
            Terrain::Nether if (0..8).contains(&section_y) => Some("minecraft:netherrack"),
            _ => None,
        }
    }

    fn top(self) -> i32 {
        match self {
            Terrain::Overworld => 192,
            Terrain::Nether => 128,
        }
    }
}

fn state(name: &str) -> VoxelId {
    VoxelId::from(blocks().0.default_state(name).0)
}

fn snapshot(col: ColumnPos, y_sections: &Arc<[i32]>, terrain: Terrain) -> FilledSnapshot {
    flat_snapshot(
        col,
        y_sections,
        |section_y| terrain.section(section_y).map(state),
        Some(terrain.top()),
    )
}

/// One column decorated by the whole ore table, counting the veins each feature
/// placed and the blocks the column and its ring took.
fn veins_of(
    program: &FeatureProgram,
    named: &[String],
    ctx: &FillContext,
    y_sections: &Arc<[i32]>,
    col: ColumnPos,
    terrain: Terrain,
) -> (BTreeMap<String, usize>, usize) {
    let snapshots: RegionSnapshots = std::array::from_fn(|slot| {
        Arc::new(snapshot(region_column(col, slot), y_sections, terrain))
    });
    let column = ColumnBlocks::from_sections(&snapshots[4].sections, y_sections);
    let mut region = ColumnRegion::new(&snapshots, &column, ctx);

    let origin = BlockPos::new(col.x * 16, ctx.router.noise.min_y, col.z * 16);
    let seed = decoration_seed(ctx.router.world_seed as i64, origin.x, origin.z);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut scratch = PlacerScratch::default();
    let mut ore_scratch = OreScratch::default();
    decorate(
        &program.present(&[0]),
        0..1,
        &|step, index| program.chain(step, index),
        &|biome, step, index| program.carries(biome, step, index),
        &mut region,
        &mut scratch,
        origin,
        seed,
        &mut |(_, index), region, rng, at, _| {
            let Some(Generator::Ore(ore)) = program.generator_at(0, index) else {
                return false;
            };
            let placed = place_modern_ore(ore, region, rng, at, &mut ore_scratch);
            if placed {
                *counts.entry(named[index].clone()).or_default() += 1;
            }
            placed
        },
    );

    let written = region
        .finish()
        .iter()
        .map(|(_, delta)| delta.writes.len())
        .sum();
    (counts, written)
}

/// The nine columns every band is measured over: one decoration seed each.
const UNITS: [ColumnPos; 9] = [
    ColumnPos::new(0, 0),
    ColumnPos::new(1, 0),
    ColumnPos::new(0, 1),
    ColumnPos::new(-7, 13),
    ColumnPos::new(13, -7),
    ColumnPos::new(100, 100),
    ColumnPos::new(-250, 371),
    ColumnPos::new(1024, -1024),
    ColumnPos::new(-3, -3),
];

/// A regression guard, not a parity claim: the per-column vein count of
/// every ore feature the corpus ships, over a fixed set of nine columns, and
/// the blocks one column and its ring take. The bands are what the first run
/// measured; a change to the placer, the chain or the vein moves them, and the
/// number to check against the reference is the harness dump, not this.
///
/// The overworld block band was re-recorded once, three blocks down: the
/// region here used to be built without the program, so `isAir` answered from
/// an empty mask and `discard_chance_on_air_exposure` never discarded anything.
#[test]
fn the_corpus_ore_counts_stay_in_the_band_the_first_run_set() {
    let (tables, named) = ore_tables();
    let program = Arc::new(ore_program(&tables));
    assert_eq!(
        tables.features.steps[0].len(),
        38,
        "the corpus's placed ore features"
    );

    let mut report: Vec<String> = Vec::new();
    for terrain in [Terrain::Overworld, Terrain::Nether] {
        let router = Arc::new(build_settings_router(terrain.settings(), 12345));
        let y_sections = dimension_y_sections(&router, -64, 24);
        let ctx = fill_context_with(router, Some(Arc::clone(&program)));

        let mut bands: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut blocks_band = (usize::MAX, 0usize);
        for col in UNITS {
            let (counts, written) = veins_of(&program, &named, &ctx, &y_sections, col, terrain);
            for feature in &named {
                let veins = counts.get(feature).copied().unwrap_or(0);
                let band = bands.entry(feature.clone()).or_insert((usize::MAX, 0));
                band.0 = band.0.min(veins);
                band.1 = band.1.max(veins);
            }
            blocks_band.0 = blocks_band.0.min(written);
            blocks_band.1 = blocks_band.1.max(written);
        }
        for (feature, (low, high)) in &bands {
            report.push(format!("{terrain:?} {feature} {low}..={high}"));
        }
        report.push(format!(
            "{terrain:?} blocks per column {}..={}",
            blocks_band.0, blocks_band.1
        ));
    }

    let recorded: Vec<&str> = RECORDED.trim().lines().map(str::trim).collect();
    assert_eq!(
        report, recorded,
        "the ore counts left the band the first run set — a regression guard, \
         not a parity claim: re-record only with the reason for the move"
    );
}

const RECORDED: &str = include_str!("fixtures/corpus_ore_bands.txt");
