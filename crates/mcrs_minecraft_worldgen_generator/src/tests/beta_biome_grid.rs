use mcrs_minecraft_worldgen_density::program::Workspace;

use super::beta_surface::build_beta_biome_source;
use super::build_beta_router;
use crate::beta_biome_grid;

#[test]
fn every_cell_is_the_biome_its_climate_answers_and_a_ring_cell_is_the_neighbour_s() {
    let router = build_beta_router();
    let (source, _) = build_beta_biome_source();
    let mut ws = Workspace::new();

    for (chunk_x, chunk_z) in [(0, 0), (3, -2)] {
        let grid = beta_biome_grid(&router, &source, chunk_x * 16, chunk_z * 16);
        for gx in 0..6 {
            for gz in 0..6 {
                let (temperature, humidity) = router.sample_beta_climate(
                    &mut ws,
                    grid.volume.block_x(gx),
                    grid.volume.block_z(gz),
                );
                let expected = source
                    .beta_biome(temperature, humidity)
                    .narrow::<u8>()
                    .expect("the biome is an id the grid can store");
                assert_eq!(
                    grid.get(gx, 0, gz),
                    expected,
                    "cell {gx},{gz} of column {chunk_x},{chunk_z}"
                );
            }
        }
    }

    let (chunk_x, chunk_z) = (-11, 6);

    let column = beta_biome_grid(&router, &source, chunk_x * 16, chunk_z * 16);
    let west = beta_biome_grid(&router, &source, (chunk_x - 1) * 16, chunk_z * 16);
    let north = beta_biome_grid(&router, &source, chunk_x * 16, (chunk_z - 1) * 16);

    for gz in 1..5 {
        assert_eq!(
            column.get(0, 0, gz),
            west.get(4, 0, gz),
            "west ring, row {gz}"
        );
    }
    for gx in 1..5 {
        assert_eq!(
            column.get(gx, 0, 0),
            north.get(gx, 0, 4),
            "north ring, column {gx}"
        );
    }
}
