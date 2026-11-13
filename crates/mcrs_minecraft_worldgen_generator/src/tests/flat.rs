use std::sync::Arc;

use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_registries::Diagnostics;
use mcrs_voxel::ColumnPos;

use super::{biome_id, blocks, test_registries};
use crate::ColumnBlocks;
use crate::flat::{FlatLayer, FlatLayers};
use crate::ids::FillIds;
use crate::multi_noise_biomes::PresetBiomeTables;
use crate::stages::{ColumnGenerator, FillContext, GeneratorSetup, fill_column, flat_y_sections};
use crate::task::CancellationToken;

#[test]
fn a_flat_dimension_fills_its_layers_with_no_router_carver_feature_or_structure() {
    let registries = test_registries();
    let mut diagnostics = Diagnostics::default();
    let fill_ids = FillIds::resolve(registries, &mut diagnostics).expect("the corpus holds ids");
    let blocks = blocks().0.clone();
    let state = |name| blocks.default_state(name).expect("a corpus block");
    let layers = FlatLayers::new(vec![
        FlatLayer {
            state: state("minecraft:bedrock"),
            height: 1,
        },
        FlatLayer {
            state: state("minecraft:dirt"),
            height: 2,
        },
    ]);
    let biome = biome_id(registries, "minecraft:plains");
    let y_sections = flat_y_sections(-64, 24);
    let ctx = FillContext::build(
        GeneratorSetup::Flat(Arc::new(layers)),
        blocks.clone(),
        y_sections.clone(),
        Some(Arc::new(BiomeSource::Fixed { biome })),
        &PresetBiomeTables::default(),
        mcrs_minecraft_heightmap::heightmap_predicates(super::blocks(), super::block_tags()),
        None,
        super::surface_ids_over(registries),
        &fill_ids,
        None,
        None,
    );
    assert!(ctx.router.is_none());
    assert!(matches!(ctx.program.generator, ColumnGenerator::Flat(_)));
    assert!(ctx.program.carvers.is_none() && ctx.program.features.is_none());
    assert!(ctx.structures.is_none());
    assert_eq!(ctx.height_context().min_y, -64);

    let mut column = ColumnBlocks::new(&y_sections);
    let snapshot = fill_column(
        &ctx,
        ColumnPos::new(3, -7),
        &mut column,
        &CancellationToken::new(),
    )
    .expect("the fill was not cancelled");
    let (palette, biomes) = snapshot.sections[0]
        .as_ref()
        .expect("the lowest section holds the layers");
    let mut states = Vec::new();
    palette.0.for_each(|state| states.push(state));
    states.sort_unstable();
    states.dedup();
    let mut wanted = [
        VoxelId::default(),
        VoxelId(state("minecraft:bedrock").number()),
        VoxelId(state("minecraft:dirt").number()),
    ];
    wanted.sort_unstable();
    assert_eq!(states, wanted);
    assert_eq!(biomes.get_cell(0, 0, 0), biomes.get_cell(15, 15, 15));
}
