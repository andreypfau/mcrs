use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_worldgen_density::aquifer_cells::AquiferCells;
use mcrs_minecraft_worldgen_density::cell::corner_bounds;

use crate::{
    CellFill, CellLattice, FillBuffers, LATTICE_PLANES, LatticePlanes, column_fluid_field,
};

use super::build_settings_router;

#[test]
fn a_lattice_node_does_not_depend_on_the_volume_around_it() {
    use glam::IVec3;
    use mcrs_minecraft_noise::sample_grid::SampleGrid;

    let router = build_settings_router("overworld", 777);
    let cell = router.cell_size().expect("the router has a cell lattice");
    let mut fill = FillBuffers::default();
    let inputs = router.cell_inputs();
    let min_y = router.noise.min_y;
    let rows = router.noise.height as i32 / cell.y + 1;

    let mut whole = CellLattice::fill(&router, 0, 0, None, &mut fill).expect("the lattice fills");
    whole.extend(&router, &mut fill);
    let ws = &mut fill.ws;
    let part = SampleGrid::new(
        IVec3::new(4, rows, 4),
        IVec3::new(cell.x, min_y, cell.z),
        cell,
    );
    let mut values = vec![0.0f32; inputs.len() * part.len()];
    router.fill_nodes(ws, &part, inputs, &mut values);

    let mut compared = 0;
    for k in 0..inputs.len() {
        for x in 0..4 {
            for z in 0..4 {
                for y in 0..rows {
                    let here = values[k * part.len() + part.index_unchecked(x, y, z)];
                    let there = whole.values
                        [k * whole.volume.len() + whole.volume.index_unchecked(x + 1, y, z + 1)];
                    assert_eq!(
                        here.to_bits(),
                        there.to_bits(),
                        "wrapper {k} at node {x},{y},{z}"
                    );
                    compared += 1;
                }
            }
        }
    }
    assert_eq!(compared, inputs.len() * 4 * 4 * rows as usize);
}

#[test]
fn a_lattice_borrowing_shorter_neighbour_planes_matches_a_lattice_filled_alone() {
    use glam::IVec3;
    use mcrs_minecraft_noise::sample_grid::SampleGrid;

    let router = build_settings_router("overworld", 845);
    let mut fill = FillBuffers::default();

    let mut west = CellLattice::fill(&router, 0, 0, None, &mut fill).expect("the lattice fills");
    let size = west.volume.size();
    west.volume = SampleGrid::new(
        IVec3::new(size.x, 3, size.z),
        west.volume.min_block(),
        west.cell,
    );
    west.compute(&router, &mut fill);
    let mut north =
        CellLattice::fill(&router, 16, -16, None, &mut fill).expect("the lattice fills");
    north.extend(&router, &mut fill);

    let mut borrowing =
        CellLattice::fill(&router, 16, 0, None, &mut fill).expect("the lattice fills");
    borrowing.extend(&router, &mut fill);

    LATTICE_PLANES.with_borrow_mut(|planes| *planes = LatticePlanes::default());
    let mut alone = CellLattice::fill(&router, 16, 0, None, &mut fill).expect("the lattice fills");
    alone.extend(&router, &mut fill);

    assert_eq!(borrowing.volume.size(), alone.volume.size());
    let differing = borrowing
        .values
        .iter()
        .zip(&alone.values)
        .filter(|(left, right)| left.to_bits() != right.to_bits())
        .count();
    assert_eq!(
        differing, 0,
        "lattice nodes differ from the lattice filled alone"
    );
}

mod exhaustive {
    use super::*;

    #[test]
    #[expect(
        clippy::excessive_nesting,
        reason = "checks the densities of each lattice cell against the bound the cell settled on, one loop per axis"
    )]
    fn a_cell_bound_contains_every_density_inside_it() {
        use glam::IVec3;
        use mcrs_minecraft_noise::sample_grid::SampleGrid;
        use mcrs_minecraft_worldgen_density::router::FINAL_DENSITY;

        const SIDE: i32 = 64;

        let router = build_settings_router("overworld", 845);
        let cell = router.cell_size().expect("the router has a cell lattice");
        let mut fill = FillBuffers::default();
        let mut dense = vec![0.0f32; (cell.x * cell.y * cell.z) as usize];
        let (mut cells, mut settled) = (0u64, 0u64);

        for (label, origin) in [("ocean", (0, 0)), ("inland", (40, 40))] {
            for i in 0..SIDE * SIDE {
                let (cx, cz) = (origin.0 + i % SIDE, origin.1 + i / SIDE);
                let mut lattice = CellLattice::fill(&router, cx * 16, cz * 16, None, &mut fill)
                    .expect("the router has a cell lattice");
                lattice.extend(&router, &mut fill);
                let mut aquifer = AquiferCells::default();
                let mut fluid = column_fluid_field(&router, &mut aquifer, cx * 16, cz * 16);
                fill.corners.resize(lattice.width, Interval::exact(0.0));
                let size = lattice.volume.size();
                for z in 0..size.z - 1 {
                    for x in 0..size.x - 1 {
                        for y in 0..size.y - 1 {
                            let at = IVec3::new(x, y, z);
                            let verdict = lattice.classify(&router, at, &mut fluid, &mut fill);
                            corner_bounds(&lattice.values, &lattice.volume, at, &mut fill.corners);
                            let min = IVec3::new(
                                lattice.volume.block_x(x),
                                lattice.volume.block_y(y),
                                lattice.volume.block_z(z),
                            );
                            let Some(bound) = router.final_density_cell_bounds(
                                &fill.corners,
                                min,
                                min + cell - 1,
                                &mut fill.cell_terms,
                            ) else {
                                continue;
                            };

                            let volume = SampleGrid::dense(cell, min);
                            router
                                .program()
                                .fill(&mut fill.ws, &volume, FINAL_DENSITY, &mut dense);

                            cells += 1;
                            for &density in dense.iter() {
                                assert!(
                                    density >= bound.min() && density <= bound.max(),
                                    "[{label}] cell at {min} of chunk {cx},{cz}: density {density:e} \
                                     outside the bound [{:e}, {:e}]",
                                    bound.min(),
                                    bound.max()
                                );
                            }
                            match verdict {
                                CellFill::Mixed => {}
                                CellFill::Solid => {
                                    settled += 1;
                                    assert!(
                                        dense.iter().all(|d| *d > 0.0),
                                        "[{label}] cell at {min} settled solid but holds void"
                                    );
                                }
                                _ => {
                                    settled += 1;
                                    assert!(
                                        dense.iter().all(|d| *d <= 0.0),
                                        "[{label}] cell at {min} settled empty but holds substance"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("checked {cells} cells, {settled} of them settled");
        assert!(settled > 0, "no cell settled, so the bound went untested");
    }
}
