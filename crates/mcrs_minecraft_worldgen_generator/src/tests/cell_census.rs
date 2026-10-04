use mcrs_minecraft_worldgen_density::cell::corner_bounds;
use mcrs_minecraft_worldgen_noise::interval::Interval;

use crate::{CellFill, CellLattice, FillBuffers, column_fluid_field};

use super::build_settings_router;

/// A lattice node's value is a function of the node, not of the volume it was
/// asked about as part of. Nothing may share a plane between neighbouring
/// columns until that holds bit for bit.
#[test]
fn a_lattice_node_does_not_depend_on_the_volume_around_it() {
    use bevy_math::IVec3;
    use mcrs_minecraft_worldgen_density::program::Workspace;
    use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;

    let router = build_settings_router("overworld", 777);
    let cell = router.cell_size().expect("the router has a cell lattice");
    let mut ws = Workspace::default();
    let inputs = router.cell_inputs();
    let min_y = router.noise.min_y;
    let rows = router.noise.height as i32 / cell.y + 1;

    let whole = CellLattice::fill(&router, 0, 0, &mut ws).expect("the lattice fills");
    let part = SampleGrid::new(
        IVec3::new(4, rows, 4),
        IVec3::new(cell.x, min_y, cell.z),
        cell,
    );
    let mut values = vec![0.0f32; inputs.len() * part.len()];
    router.fill_nodes(&mut ws, &part, inputs, &mut values);

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

mod exhaustive {
    use super::*;

    /// `docs/worldgen.md` R6. A cell bound that is understated writes stone through
    /// air, and no test that is not written against it would notice: the fill does
    /// not crash, does not slow down, and the wrong blocks look like terrain.
    ///
    /// So, over a region of both ocean and land: every bound must contain every
    /// density in its cell, and every cell the bound settles must hold only
    /// densities of the sign it settled on.
    #[test]
    fn a_cell_bound_contains_every_density_inside_it() {
        use bevy_math::IVec3;
        use mcrs_minecraft_worldgen_density::router::FINAL_DENSITY;
        use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;

        /// Chunks per side, per region. Widening this is the whole knob.
        const SIDE: i32 = 64;

        let router = build_settings_router("overworld", 845);
        let cell = router.cell_size().expect("the router has a cell lattice");
        let mut fill = FillBuffers::default();
        let mut dense = vec![0.0f32; (cell.x * cell.y * cell.z) as usize];
        let (mut cells, mut settled) = (0u64, 0u64);

        for (label, origin) in [("ocean", (0, 0)), ("inland", (40, 40))] {
            for i in 0..SIDE * SIDE {
                let (cx, cz) = (origin.0 + i % SIDE, origin.1 + i / SIDE);
                let lattice = CellLattice::fill(&router, cx * 16, cz * 16, &mut fill.ws)
                    .expect("the router has a cell lattice");
                let mut fluid = column_fluid_field(&router, cx * 16, cz * 16);
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

                            // The lattice is pinned, so this is the interpolation
                            // the production fill would have done for these blocks.
                            let volume = SampleGrid::dense(cell, min);
                            router
                                .program
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
