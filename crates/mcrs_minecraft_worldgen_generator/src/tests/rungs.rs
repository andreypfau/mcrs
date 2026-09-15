//! What a rung buys: a feature reads the ground every other column already
//! finished shaping, rather than the ground as it stood before any of them ran.

use std::collections::BTreeMap;
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_worldgen_feature::compile::{CompiledPlacedFeature, FeatureSteps};
use mcrs_minecraft_worldgen_feature::placement::PlacementModifier;
use mcrs_minecraft_worldgen_feature::proto::{Holder, PlacedFeature};

use crate::features::FeatureTables;
use crate::staging::FilledSnapshot;

use super::{TEMPERATE, blocks, generate_region};

const BIOME: &str = "minecraft:badlands";
const SEED: u64 = 0x1a2e;

/// The lakes step, with the one-in-two-hundred filter of the shipped placement
/// dropped so that every column digs one. Everything else is
/// `minecraft:lake_lava_surface` as it ships: a sixteen-wide blob of lava under
/// four of cave air, walled in stone, straddling whatever column boundary it
/// lands on.
const LAKE: &str = r#"[
  { "type": "minecraft:in_square" },
  { "type": "minecraft:heightmap", "heightmap": "WORLD_SURFACE_WG" },
  { "type": "minecraft:biome" }
]"#;

/// The vegetation step of the badlands, over `MOTION_BLOCKING` as the shipped
/// dry grass reads it, and with the count raised so one region is a census
/// rather than a handful of draws.
const PLANTS: &str = r#"[
  { "type": "minecraft:count", "count": 48 },
  { "type": "minecraft:in_square" },
  { "type": "minecraft:heightmap", "heightmap": "MOTION_BLOCKING" },
  { "type": "minecraft:biome" },
  { "type": "minecraft:block_predicate_filter",
    "predicate": { "type": "minecraft:matching_block_tag", "tag": "minecraft:air" } }
]"#;

/// The two features at the steps the corpus puts them at, so that the rungs the
/// program cuts are the ones the shipped biomes get.
fn tables() -> FeatureTables {
    const LAKES: usize = 1;
    const VEGETAL: usize = 9;
    const STEPS: usize = 11;

    let entry = |id: &str, placement: &str| {
        Arc::new(CompiledPlacedFeature {
            id: Some(ResourceLocation::parse(id).unwrap()),
            placed: PlacedFeature {
                feature: Holder::Reference(ResourceLocation::parse(id).unwrap()),
                placement: serde_json::from_str::<Vec<PlacementModifier>>(placement)
                    .expect("the placement parses"),
            },
        })
    };

    let mut steps: Vec<Vec<Arc<CompiledPlacedFeature>>> = vec![Vec::new(); STEPS];
    steps[LAKES].push(entry("minecraft:lake_lava", LAKE));
    steps[VEGETAL].push(entry("minecraft:dead_bush", PLANTS));

    let mut next_token = 0;
    let token: Vec<Vec<usize>> = steps
        .iter()
        .map(|step| {
            step.iter()
                .map(|_| {
                    next_token += 1;
                    next_token - 1
                })
                .collect()
        })
        .collect();
    let carried: Vec<FixedBitSet> = steps
        .iter()
        .map(|step| {
            let mut bits = FixedBitSet::with_capacity(step.len());
            bits.insert_range(..);
            bits
        })
        .collect();

    FeatureTables {
        features: FeatureSteps {
            steps,
            token,
            per_biome: vec![carried],
        },
        biome_order: vec![ResourceLocation::parse(BIOME).unwrap()],
        climate: BTreeMap::from([(ResourceLocation::parse(BIOME).unwrap(), TEMPERATE)]),
    }
}

fn state_of(name: &str) -> VoxelId {
    VoxelId::from(blocks().0.default_state(name).0)
}

fn named(state: VoxelId) -> &'static str {
    let index = blocks().0.block_index(BlockStateId(state.0));
    blocks().0.blocks()[index as usize].identifier.as_str()
}

fn block_at(column: &FilledSnapshot, x: usize, y: i32, z: usize) -> VoxelId {
    let Some(slot) = column.slot(y) else {
        return VoxelId::default();
    };
    column.sections[slot]
        .as_ref()
        .map_or(VoxelId::default(), |(palette, _)| {
            palette.0.get(x, (y & 0xF) as usize, z)
        })
}

/// A lava lake is dug at the lakes step and the plants grow at the vegetal one,
/// so by the time a plant asks whether it can stand somewhere, every lake in
/// reach of it — its own column's and its neighbours' — is already in the
/// ground it reads.
///
/// Without the rung between the two steps a column cannot see a neighbour's
/// lake at all: it plants on the terracotta that was there when the region was
/// filled, the lake replaces that terracotta at the merge, and the plant is
/// left standing on lava or hanging over the hole.
#[test]
fn a_plant_never_grows_on_a_lake_a_neighbour_dug() {
    let (ctx, _) = super::trees::dimension_over(BIOME, Arc::new(tables()), SEED);
    assert_eq!(
        ctx.rungs(),
        2,
        "the lakes step and the vegetal step are cut into rungs of their own"
    );

    let region = generate_region(&ctx, ColumnPos::new(-1, -1), ColumnPos::new(1, 1));
    let lava = state_of("minecraft:lava");
    let plant = state_of("minecraft:dead_bush");

    let supports = super::trees::tag_states("minecraft:supports_dry_vegetation");
    let (mut lakes, mut plants, mut unsupported) = (0usize, 0usize, Vec::new());
    for column in region.values() {
        let bottom = column.y_sections[0] * 16;
        let top = bottom + column.y_sections.len() as i32 * 16;
        for y in bottom..top {
            for x in 0..16 {
                for z in 0..16 {
                    let state = block_at(column, x, y, z);
                    if state == lava {
                        lakes += 1;
                    }
                    if state != plant {
                        continue;
                    }
                    plants += 1;
                    let below = block_at(column, x, y - 1, z);
                    if !supports.contains(below.0 as usize) {
                        unsupported.push((column.col, x, y, z, named(below)));
                    }
                }
            }
        }
    }

    assert!(lakes > 0, "no lake was dug, so nothing was under test");
    assert!(plants > 0, "no plant grew, so nothing was under test");
    let sample: Vec<_> = unsupported.iter().take(8).collect();
    assert!(
        unsupported.is_empty(),
        "{} of {plants} plants stand on nothing that holds them, e.g. {sample:?}",
        unsupported.len()
    );
}
