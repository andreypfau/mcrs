use crate::vanilla_parity::overworld_router;
use glam::IVec3;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::cell::CELL_BOUNDS_SLACK;
use mcrs_minecraft_worldgen_density::column_bound::ColumnBoundScratch;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use std::sync::LazyLock;

static ROUTER: LazyLock<NoiseRouter> = LazyLock::new(|| overworld_router(845));

fn assert_rows_above_first_air_are_air(chunks: impl Iterator<Item = (i32, i32)>) {
    let router = &*ROUTER;
    let cell = router.cell_size().unwrap();
    let noise = router.noise;
    let mut ws = Workspace::new();
    let mut scratch = ColumnBoundScratch::default();
    let mut skipped = 0;
    for (chunk_x, chunk_z) in chunks {
        let lattice = SampleGrid::new(
            IVec3::new(
                16 / cell.x + 1,
                noise.height as i32 / cell.y + 1,
                16 / cell.z + 1,
            ),
            IVec3::new(chunk_x * 16, noise.min_y, chunk_z * 16),
            cell,
        );
        let inputs = router.cell_inputs();
        let mut values = vec![0.0f32; inputs.len() * lattice.len()];
        router.fill_nodes(&mut ws, &lattice, inputs, &mut values);

        let first_air = router.final_density_first_air_row(&mut ws, &lattice, None, &mut scratch);
        let top = lattice.size().y - 1;
        for hint in [0, first_air / 2, first_air, top] {
            assert_eq!(
                router.final_density_first_air_row(&mut ws, &lattice, Some(hint), &mut scratch),
                first_air,
                "chunk ({chunk_x}, {chunk_z}) with hint {hint}"
            );
        }
        let buried = SampleGrid::new(
            IVec3::new(lattice.size().x, 3, lattice.size().z),
            lattice.min_block(),
            cell,
        );
        let buried_top = buried.size().y - 1;
        assert_eq!(
            router.final_density_first_air_row(&mut ws, &buried, Some(buried_top), &mut scratch),
            router.final_density_first_air_row(&mut ws, &buried, None, &mut scratch),
            "chunk ({chunk_x}, {chunk_z}) buried lattice with the hint at its top"
        );
        for cell_y in first_air..top {
            for cell_z in 0..lattice.size().z - 1 {
                for cell_x in 0..lattice.size().x - 1 {
                    let at = IVec3::new(cell_x, cell_y, cell_z);
                    let min = IVec3::new(
                        lattice.block_x(cell_x),
                        lattice.block_y(cell_y),
                        lattice.block_z(cell_z),
                    );
                    let chain = router
                        .final_density_chain_bound(&values, &lattice, at, min, min + cell - 1)
                        .unwrap();
                    assert!(
                        chain.max() < -CELL_BOUNDS_SLACK,
                        "chunk ({chunk_x}, {chunk_z}) cell {at}: chain bound {chain:?} is not air"
                    );
                }
            }
        }
        skipped += top - first_air;
    }
    assert!(skipped > 0, "the bound skipped no row");
}

#[test]
fn every_cell_above_the_first_air_row_is_air_by_the_lattice_chain_bound() {
    assert_rows_above_first_air_are_air([(0, 0), (-7, 3), (12, -20)].into_iter());
}

mod exhaustive {
    use super::*;

    #[test]
    fn every_cell_above_the_first_air_row_is_air_over_a_region() {
        assert_rows_above_first_air_are_air(
            (-24..24)
                .step_by(3)
                .flat_map(|x| (-24..24).step_by(3).map(move |z| (x, z))),
        );
    }
}

#[test]
fn the_cell_inputs_seeded_by_the_column_bound_match_an_unseeded_fill() {
    let router = &*ROUTER;
    let cell = router.cell_size().unwrap();
    let noise = router.noise;
    let inputs = router.cell_inputs();
    let mut ws = Workspace::new();
    let mut scratch = ColumnBoundScratch::default();
    for (chunk_x, chunk_z) in [(0, 0), (-7, 3), (35, 0)] {
        let min = IVec3::new(chunk_x * 16, noise.min_y, chunk_z * 16);
        let size = IVec3::new(
            16 / cell.x + 1,
            noise.height as i32 / cell.y + 1,
            16 / cell.z + 1,
        );
        let lattice = SampleGrid::new(size, min, cell);
        router.final_density_first_air_row(&mut ws, &lattice, None, &mut scratch);
        let inner = SampleGrid::new(
            IVec3::new(size.x - 1, size.y / 2, size.z - 1),
            min + IVec3::new(cell.x, cell.y * 3, cell.z),
            cell,
        );
        for volume in [lattice, inner] {
            let mut expected = vec![0.0f32; inputs.len() * volume.len()];
            ws.take_count();
            router.fill_nodes(&mut ws, &volume, inputs, &mut expected);
            let unseeded = ws.take_count();
            let mut seeded = vec![0.0f32; expected.len()];
            router.fill_cell_inputs(&mut ws, &volume, &scratch, &mut seeded);
            assert!(
                ws.take_count().evaluated < unseeded.evaluated,
                "chunk ({chunk_x}, {chunk_z}) volume {volume:?} took the unseeded fill"
            );
            let bits = |values: &[f32]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
            assert_eq!(
                bits(&seeded),
                bits(&expected),
                "chunk ({chunk_x}, {chunk_z}) volume {volume:?}"
            );
        }
    }
}

#[test]
fn a_seed_prepared_by_another_router_is_not_used() {
    let prepared_by = overworld_router(846);
    let router = &*ROUTER;
    let cell = router.cell_size().unwrap();
    let noise = router.noise;
    let lattice = SampleGrid::new(
        IVec3::new(
            16 / cell.x + 1,
            noise.height as i32 / cell.y + 1,
            16 / cell.z + 1,
        ),
        IVec3::new(0, noise.min_y, 0),
        cell,
    );
    let inputs = router.cell_inputs();
    let mut ws = Workspace::new();
    let mut scratch = ColumnBoundScratch::default();
    prepared_by.final_density_first_air_row(&mut ws, &lattice, None, &mut scratch);
    let mut expected = vec![0.0f32; inputs.len() * lattice.len()];
    router.fill_nodes(&mut ws, &lattice, inputs, &mut expected);
    let mut filled = vec![0.0f32; expected.len()];
    router.fill_cell_inputs(&mut ws, &lattice, &scratch, &mut filled);
    let bits = |values: &[f32]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&filled), bits(&expected));
}
