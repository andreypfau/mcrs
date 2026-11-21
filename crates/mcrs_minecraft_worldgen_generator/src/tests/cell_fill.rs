use crate::ColumnBiomes;
use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen::beard::Beard;
use mcrs_minecraft_worldgen_density::aquifer::point_barrier;
use mcrs_minecraft_worldgen_density::aquifer_cells::AquiferCells;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::FINAL_DENSITY;
use mcrs_voxel::ColumnPos;

use crate::task::CancellationToken;
use crate::{ColumnBlocks, NO_TOP, column_fluid_field, cover_fill_group, fill_column_dense_any};

use super::build_settings_router as build_router;
use super::ladder::structure_dimension;

#[test]
fn cell_elimination_matches_the_block_by_block_fill() {
    let router = build_router("overworld", 845);
    let y_sections: Vec<i32> = (-4..20).collect();
    let (section_x, section_z) = (3, -7);
    let mut column = ColumnBlocks::new(&y_sections);
    let mut cells = AquiferCells::default();
    let filled = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        &y_sections,
        &router,
        &mut cells,
        &ColumnBiomes::None,
        None,
        None,
        &CancellationToken::new(),
    )
    .expect("the column is not cancelled");

    let stone = router.default_block_state;
    let mut ws = Workspace::new();
    let mut cells = AquiferCells::default();
    let mut oracle = column_fluid_field(&router, &mut cells, section_x * 16, section_z * 16);
    let mut barrier_ws = Workspace::new();
    let mut barrier = point_barrier(&router, &mut barrier_ws);
    let mut checked = 0usize;

    for &section_y in &y_sections {
        let volume = SampleGrid::dense(
            IVec3::splat(16),
            IVec3::new(section_x * 16, section_y * 16, section_z * 16),
        );
        let mut density = vec![0.0f32; volume.len()];
        router
            .program()
            .fill(&mut ws, &volume, FINAL_DENSITY, &mut density);
        for z in 0..16 {
            for x in 0..16 {
                for y in 0..16 {
                    let at = IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                    let d = f64::from(density[volume.index_unchecked(x, y, z)]);
                    let expected = oracle
                        .substance(at.x, at.y, at.z, d, &mut barrier)
                        .unwrap_or(stone);
                    let got = column.get(x, at.y, z).expect("the section exists");
                    assert_eq!(
                        got, expected,
                        "block at {at} differs from the block-by-block fill"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 24 * 16 * 16 * 16);

    let bottom = y_sections[0] * 16;
    let top = y_sections[y_sections.len() - 1] * 16 + 15;
    let mut settled = 0usize;
    for z in 0..16 {
        for x in 0..16 {
            let expected = (bottom..=top)
                .rev()
                .find(|&y| matches!(column.get(x, y, z), Some(id) if id != VoxelId(0)))
                .unwrap_or(NO_TOP);
            assert_eq!(
                filled.tops[(z * 16 + x) as usize],
                expected,
                "strip ({x}, {z})"
            );
            settled += (expected != NO_TOP) as usize;
        }
    }
    assert_eq!(
        settled, 256,
        "every strip of an overworld column holds blocks"
    );
}

#[test]
fn a_bearded_column_matches_the_block_by_block_fill_of_the_summed_density() {
    bearded_columns_match_the_block_by_block_fill_of_the_summed_density(0);
}

mod exhaustive {
    #[test]
    fn a_bearded_column_matches_the_block_by_block_fill_of_the_summed_density() {
        super::bearded_columns_match_the_block_by_block_fill_of_the_summed_density(1);
    }
}

fn bearded_columns_match_the_block_by_block_fill_of_the_summed_density(radius: i32) {
    let dim = structure_dimension("minecraft:village_plains", "minecraft:plains");
    let index = dim
        .ctx
        .structures
        .as_deref()
        .expect("a structure dimension carries its index");
    let router = dim.ctx.router.as_deref().expect("a noise context");
    let y_sections = &dim.ctx.y_sections[..];
    let stone = router.default_block_state;
    let (mut checked, mut raised, mut carved) = (0usize, 0usize, 0usize);

    for (dx, dz) in (-radius..=radius).flat_map(|dx| (-radius..=radius).map(move |dz| (dx, dz))) {
        let col = ColumnPos::new(dim.centre.x + dx, dim.centre.z + dz);
        let beard = index
            .beard(col)
            .unwrap_or_else(|| panic!("{col:?} beside the village carries no beard"));
        let fill = |beard: Option<&Beard>| {
            let mut column = ColumnBlocks::new(y_sections);
            let mut cells = AquiferCells::default();
            fill_column_dense_any(
                &mut column,
                col.x,
                col.z,
                y_sections,
                router,
                &mut cells,
                &ColumnBiomes::None,
                None,
                beard,
                &CancellationToken::new(),
            )
            .expect("the column is not cancelled");
            column
        };
        let bearded = fill(Some(&beard));
        let bare = fill(None);

        let mut ws = Workspace::new();
        let mut cells = AquiferCells::default();
        let mut oracle = column_fluid_field(router, &mut cells, col.x * 16, col.z * 16);
        let mut barrier_ws = Workspace::new();
        let mut barrier = point_barrier(router, &mut barrier_ws);
        for &section_y in y_sections {
            let volume = SampleGrid::dense(
                IVec3::splat(16),
                IVec3::new(col.x * 16, section_y * 16, col.z * 16),
            );
            let mut density = vec![0.0f32; volume.len()];
            router
                .program()
                .fill(&mut ws, &volume, FINAL_DENSITY, &mut density);
            for z in 0..16 {
                for x in 0..16 {
                    for y in 0..16 {
                        let at =
                            IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                        let summed = density[volume.index_unchecked(x, y, z)]
                            + beard.sample(at.x, at.y, at.z);
                        let expected = oracle
                            .substance(at.x, at.y, at.z, f64::from(summed), &mut barrier)
                            .unwrap_or(stone);
                        let got = bearded.get(x, at.y, z).expect("the section exists");
                        assert_eq!(
                            got, expected,
                            "block at {at} differs from the block-by-block fill of the summed density"
                        );
                        let without = bare.get(x, at.y, z).expect("the section exists");
                        raised += usize::from(without == VoxelId(0) && got == stone);
                        carved += usize::from(without == stone && got != stone);
                        checked += 1;
                    }
                }
            }
        }
    }
    let columns = ((2 * radius + 1) * (2 * radius + 1)) as usize;
    assert_eq!(checked, columns * y_sections.len() * 16 * 16 * 16);
    assert!(
        raised > 0,
        "the beard raised no air to stone around the village"
    );
    assert!(carved > 0, "the beard carved no stone around the village");
}

#[test]
fn a_fill_group_sharing_its_aquifer_cells_fills_each_column_as_alone() {
    let router = build_router("overworld", 845);
    let y_sections: Vec<i32> = (-4..20).collect();
    let group: Vec<ColumnPos> = (-8..-4)
        .flat_map(|z| (0..4).map(move |x| ColumnPos::new(x, z)))
        .collect();
    let blocks = |col: ColumnPos, cells: &mut AquiferCells| {
        let mut column = ColumnBlocks::new(&y_sections);
        fill_column_dense_any(
            &mut column,
            col.x,
            col.z,
            &y_sections,
            &router,
            cells,
            &ColumnBiomes::None,
            None,
            None,
            &CancellationToken::new(),
        )
        .expect("the column is not cancelled");
        let mut states = Vec::new();
        for y in -64..320 {
            for z in 0..16 {
                states.extend((0..16).map(|x| column.get(x, y, z)));
            }
        }
        states
    };

    let mut shared = AquiferCells::default();
    cover_fill_group(&router, group.iter().copied(), &mut shared);
    for &col in &group {
        let grouped = blocks(col, &mut shared);
        let alone = blocks(col, &mut AquiferCells::default());
        assert!(
            grouped == alone,
            "column {col:?} differs when its group shares the aquifer cells"
        );
    }
}
