use crate::vanilla_parity::overworld_router;
use glam::IVec3;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::cell::CELL_BOUNDS_SLACK;
use mcrs_minecraft_worldgen_density::column_bound::ColumnScratch;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use std::sync::LazyLock;

static ROUTER: LazyLock<NoiseRouter> = LazyLock::new(|| overworld_router(845));

fn assert_rows_above_first_air_are_air(chunks: impl Iterator<Item = (i32, i32)>) {
    let router = &*ROUTER;
    let cell = router.cell_size().unwrap();
    let noise = router.noise;
    let mut ws = Workspace::new();
    let mut scratch = ColumnScratch::default();
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
