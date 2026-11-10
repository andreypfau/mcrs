use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_worldgen_density::aquifer::{FluidField, point_barrier};
use mcrs_minecraft_worldgen_density::compile::build_router;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    FINAL_DENSITY, NoiseGeneratorSettings, NoiseRouter, RouterBlocks,
};
use mcrs_minecraft_worldgen_density::surface_levels::SurfaceLevels;
use mcrs_minecraft_worldgen_testing as corpus;

const BLOCKS: RouterBlocks = RouterBlocks {
    default_block: VoxelId(1),
    default_fluid: VoxelId(2),
    water: VoxelId(2),
    lava: VoxelId(3),
};

fn overworld(seed: u64, edit: impl FnOnce(&mut serde_json::Value)) -> NoiseRouter {
    let mut raw: serde_json::Value =
        corpus::read("noise_settings", &rl!("minecraft:overworld").to_buf());
    edit(&mut raw);
    let settings: NoiseGeneratorSettings = serde_json::from_value(raw).unwrap();
    build_router(
        &settings,
        &corpus::registry("density_function"),
        &corpus::registry("noise"),
        seed,
        BLOCKS,
    )
    .unwrap()
}

/// A coast, a mountain range and deep inland, so the volumes seen mix sea
/// cells with dry ones, hold perched lakes, and reach lava depths.
const COLUMNS: [(i32, i32); 3] = [(0, 0), (52, 36), (-118, -119)];

fn column_volume(router: &NoiseRouter, chunk_x: i32, chunk_z: i32) -> SampleGrid {
    SampleGrid::dense(
        IVec3::new(16, router.noise.height as i32, 16),
        IVec3::new(chunk_x * 16, router.noise.min_y, chunk_z * 16),
    )
}

/// Every shortcut is an exact rewrite of the search: the lemmas, per block and
/// per cell, answer what the oracle answers, for the substance and the tick.
#[test]
fn lemmas_agree_with_the_search_block_for_block() {
    check_parity(&overworld(42, |_| {}));
}

mod exhaustive {
    use super::*;

    #[test]
    fn lemmas_agree_with_the_search_block_for_block() {
        for seed in [42u64, 2] {
            let router = overworld(seed, |_| {});
            check_parity(&router);
        }
    }
}

/// A surface level is a function of its quart column, not of the volume it
/// is sampled as part of. The field samples each level alone and a worker
/// keeps it for other fields, and neither holds unless this does bit for bit.
#[test]
fn a_surface_level_does_not_depend_on_the_volume_around_it() {
    let router = overworld(42, |_| {});
    let root = router.aquifer.as_ref().unwrap().surface_level;
    let mut ws = Workspace::new();
    let mut compared = 0usize;
    for (chunk_x, chunk_z) in COLUMNS {
        let grid = SampleGrid::new(
            IVec3::new(27, 1, 19),
            IVec3::new(chunk_x * 16 - 48, 0, chunk_z * 16 - 16),
            IVec3::new(4, 1, 4),
        );
        let mut together = vec![0.0f32; grid.len()];
        router.program.fill(&mut ws, &grid, root, &mut together);
        for z in 0..grid.size().z {
            for x in 0..grid.size().x {
                let at = IVec3::new(grid.block_x(x), 0, grid.block_z(z));
                let mut alone = [0.0f32];
                router
                    .program
                    .fill(&mut ws, &SampleGrid::point(at), root, &mut alone);
                assert_eq!(
                    alone[0].to_bits(),
                    together[grid.index_unchecked(x, 0, z)].to_bits(),
                    "quart column at {at}"
                );
                compared += 1;
            }
        }
    }
    assert_eq!(compared, COLUMNS.len() * 27 * 19);
}

/// A field that starts from the levels a worker kept answers what a field that
/// samples every level itself answers: beside the column that left them, a
/// table's width away where the same slots are taken, and under another seed.
#[test]
fn kept_surface_levels_do_not_change_a_field() {
    let first = overworld(42, |_| {});
    let second = overworld(2, |_| {});
    let mut kept = SurfaceLevels::default();
    let mut checked = 0usize;
    for (router, (chunk_x, chunk_z)) in [
        (&first, (0, 0)),
        (&first, (1, 0)),
        (&first, (32, 0)),
        (&first, (1, 0)),
        (&second, (1, 0)),
        (&first, (0, 0)),
    ] {
        let volume = column_volume(router, chunk_x, chunk_z);
        let (min, max) = (volume.min_block(), volume.max_block());
        let mut warm = FluidField::new(router, &mut kept, min, max);
        let mut cold = FluidField::new(router, &mut SurfaceLevels::default(), min, max);
        let mut warm_ws = Workspace::new();
        let mut cold_ws = Workspace::new();
        for z in (min.z..=max.z).step_by(3) {
            for x in (min.x..=max.x).step_by(3) {
                for y in min.y..=max.y {
                    let got =
                        warm.substance(x, y, z, 0.0, &mut point_barrier(router, &mut warm_ws));
                    let want =
                        cold.substance(x, y, z, 0.0, &mut point_barrier(router, &mut cold_ws));
                    assert_eq!(got, want, "block {x},{y},{z} of column {chunk_x},{chunk_z}");
                    checked += 1;
                }
            }
        }
        warm.keep_surface_levels(&mut kept);
    }
    assert_eq!(checked, 6 * 6 * 6 * first.noise.height as usize);
}

/// A barrier twice as loud widens the shell and must still leave the lemmas
/// exact: the margins come from the interval, not from a constant.
#[test]
fn a_louder_barrier_widens_the_shell_and_stays_exact() {
    let quiet = overworld(42, |_| {});
    let loud = overworld(42, |raw| {
        let barrier = raw["aquifers"]["barrier"].take();
        raw["aquifers"]["barrier"] = serde_json::json!({
            "type": "minecraft:mul",
            "left": 2.0,
            "right": barrier,
        });
    });
    let margins = |router: &NoiseRouter| {
        let aquifer = router.aquifer.as_ref().unwrap();
        (aquifer.margin_above, aquifer.margin_below)
    };
    assert_eq!(margins(&quiet), (5, 23));
    assert_eq!(margins(&loud), (5, 24));
    check_parity(&loud);
}

#[expect(
    clippy::excessive_nesting,
    reason = "compares the aquifer of every cell of the region with the reference across three axes, so a mismatch names its cell"
)]
fn check_parity(router: &NoiseRouter) {
    let mut ws = Workspace::new();
    let cell = router.cell_size().unwrap();
    let mut checked = 0usize;
    let mut settled_cells = 0usize;
    for (chunk_x, chunk_z) in COLUMNS {
        let volume = column_volume(router, chunk_x, chunk_z);
        let mut density = vec![0.0f32; volume.len()];
        router
            .program
            .fill(&mut ws, &volume, FINAL_DENSITY, &mut density);
        let mut oracle = FluidField::new(
            router,
            &mut SurfaceLevels::default(),
            volume.min_block(),
            volume.max_block(),
        );
        let mut fast = FluidField::new(
            router,
            &mut SurfaceLevels::default(),
            volume.min_block(),
            volume.max_block(),
        );
        let mut oracle_ws = Workspace::new();
        let mut fast_ws = Workspace::new();

        for z in 0..volume.size().z {
            for x in 0..volume.size().x {
                for y in 0..volume.size().y {
                    let at = IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                    let d = f64::from(density[volume.index_unchecked(x, y, z)]);
                    for d in [d.min(0.0), 0.0] {
                        let want = oracle.substance(
                            at.x,
                            at.y,
                            at.z,
                            d,
                            &mut point_barrier(router, &mut oracle_ws),
                        );
                        let got = fast.substance_settled(
                            at.x,
                            at.y,
                            at.z,
                            d,
                            &mut point_barrier(router, &mut fast_ws),
                        );
                        assert_eq!(got, want, "block {at} with d={d}");
                        checked += 1;
                    }
                }
            }
        }

        // The cell answers, against the oracle over every block of the cell.
        for cz in 0..volume.size().z / cell.z {
            for cx in 0..volume.size().x / cell.x {
                for cy in 0..volume.size().y / cell.y {
                    let min = volume.min_block() + IVec3::new(cx, cy, cz) * cell;
                    let max = min + cell - IVec3::ONE;
                    let Some(settled) = fast.settle(min, max) else {
                        continue;
                    };
                    settled_cells += 1;
                    for z in min.z..=max.z {
                        for x in min.x..=max.x {
                            for y in min.y..=max.y {
                                let want = oracle.substance(
                                    x,
                                    y,
                                    z,
                                    0.0,
                                    &mut point_barrier(router, &mut oracle_ws),
                                );
                                assert_eq!(
                                    Some(settled.at(y)),
                                    want,
                                    "cell {min}..{max} settled {settled:?}, block ({x}, {y}, {z})"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 2 * 3 * 16 * 16 * 384);
    assert!(settled_cells > 1500, "only {settled_cells} cells settled");
}
