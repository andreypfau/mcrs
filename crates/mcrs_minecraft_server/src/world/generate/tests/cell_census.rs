use mcrs_minecraft_worldgen::interval::Interval;
use mcrs_minecraft_worldgen::router::NoiseRouter;

use crate::world::generate::{CellFill, CellLattice, FillBuffers};

use super::build_settings_router;

/// How many of a chunk's cells the corner bounds settle without any per-block
/// density evaluation, and what that leaves for the block-by-block fill.
fn census(label: &str, router: &NoiseRouter, columns: i32) {
    let mut counts = [0usize; 5];
    let mut fill = FillBuffers::default();
    let sea_level = router.sea_level();
    for i in 0..columns {
        let (cx, cz) = (i % 8, i / 8);
        let lattice = CellLattice::fill(router, cx * 16, cz * 16, &mut fill.ws)
            .expect("the router has a cell lattice");
        fill.corners.resize(lattice.width, Interval::exact(0.0));
        for z in 0..lattice.volume.size().z - 1 {
            for x in 0..lattice.volume.size().x - 1 {
                for y in 0..lattice.volume.size().y - 1 {
                    let at = bevy_math::IVec3::new(x, y, z);
                    let slot = match lattice.classify(router, at, sea_level, &mut fill) {
                        CellFill::Solid => 0,
                        CellFill::Fluid => 1,
                        CellFill::Air => 2,
                        CellFill::Sea => 3,
                        CellFill::Mixed => 4,
                    };
                    counts[slot] += 1;
                }
            }
        }
    }
    let lattice = CellLattice::fill(router, 0, 0, &mut fill.ws).unwrap();
    println!(
        "[{label}] lattice: {} wrappers x {} corners = {} evaluations/chunk",
        lattice.width,
        lattice.volume.len(),
        lattice.width * lattice.volume.len(),
    );
    let total = counts.iter().sum::<usize>();
    let per_chunk = total as f64 / columns as f64;
    let settled = counts[0] + counts[1] + counts[2] + counts[3];
    let cell = router.cell_size().expect("the router has a cell lattice");
    let blocks_per_cell = (cell.x * cell.y * cell.z) as usize;
    println!(
        "[{label}] {columns} chunks: {per_chunk} cells/chunk, settled {:.1} ({:.2}%) solid={} fluid={} air={} sea={} mixed={}",
        settled as f64 / columns as f64,
        100.0 * settled as f64 / total as f64,
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        counts[4],
    );
    println!(
        "[{label}] per-block density evaluations: {} of {} ({:.2}% avoided)",
        counts[4] * blocks_per_cell,
        total * blocks_per_cell,
        100.0 * settled as f64 / total as f64,
    );
}

#[test]
#[ignore]
fn cell_elimination_census() {
    census("overworld", &build_settings_router("overworld", 845), 16);
}

/// A lattice node's value is a function of the node, not of the volume it was
/// asked about as part of. Nothing may share a plane between neighbouring
/// columns until that holds bit for bit.
#[test]
fn a_lattice_node_does_not_depend_on_the_volume_around_it() {
    use bevy_math::IVec3;
    use mcrs_minecraft_worldgen::program::Workspace;
    use mcrs_minecraft_worldgen::volume::Volume;

    let router = build_settings_router("overworld", 777);
    let cell = router.cell_size().expect("the router has a cell lattice");
    let mut ws = Workspace::default();
    let inputs = router.cell_inputs();
    let min_y = router.noise_min_y();
    let rows = router.noise_height() as i32 / cell.y + 1;

    let whole = CellLattice::fill(&router, 0, 0, &mut ws).expect("the lattice fills");
    let part = Volume::new(
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
