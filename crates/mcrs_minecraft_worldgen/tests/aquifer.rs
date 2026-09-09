use bevy_math::IVec3;
use mcrs_minecraft_worldgen::aquifer::{FluidField, point_barrier};
use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::corpus;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::{
    FINAL_DENSITY, NoiseGeneratorSettings, NoiseRouter, RouterBlocks,
};
use mcrs_minecraft_worldgen::volume::Volume;
use mcrs_voxel_storage::VoxelId;

const BLOCKS: RouterBlocks = RouterBlocks {
    default_block: VoxelId(1),
    default_fluid: VoxelId(2),
    water: VoxelId(2),
    lava: VoxelId(3),
};

fn overworld(seed: u64, edit: impl FnOnce(&mut serde_json::Value)) -> NoiseRouter {
    let path = corpus::worldgen_dir().join("noise_settings/overworld.json");
    let mut raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    edit(&mut raw);
    let settings: NoiseGeneratorSettings = serde_json::from_value(raw).unwrap();
    build_router(
        &settings,
        &corpus::registry("density_function"),
        &corpus::registry("noise"),
        seed,
        BLOCKS,
        None,
    )
    .unwrap()
}

/// A coast, a mountain range and deep inland, so the windows seen mix sea
/// cells with dry ones, hold perched lakes, and reach lava depths.
const COLUMNS: [(i32, i32); 3] = [(0, 0), (52, 36), (-118, -119)];

fn column_volume(router: &NoiseRouter, chunk_x: i32, chunk_z: i32) -> Volume {
    Volume::dense(
        IVec3::new(16, router.noise.height as i32, 16),
        IVec3::new(chunk_x * 16, router.noise.min_y, chunk_z * 16),
    )
}

/// Every shortcut is an exact rewrite of the search: the lemmas, per block and
/// per cell, answer what the oracle answers, for the substance and the tick.
#[test]
fn lemmas_agree_with_the_search_block_for_block() {
    for seed in [42u64, 2] {
        let router = overworld(seed, |_| {});
        check_parity(&router);
    }
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
        let mut oracle = FluidField::new(router, volume.min_block(), volume.max_block());
        let mut fast = FluidField::new(router, volume.min_block(), volume.max_block());
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
