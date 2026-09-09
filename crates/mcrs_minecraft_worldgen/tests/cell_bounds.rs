//! A cell the bound settles is filled with one block state outright, so a bound
//! that does not contain every density inside its cell writes stone through air.

use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_voxel_storage::VoxelId;
use std::collections::BTreeMap;

use mcrs_minecraft_worldgen::cell::CellBounds;
use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::corpus;
use mcrs_minecraft_worldgen::interval::Interval;
use mcrs_minecraft_worldgen::material::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
};
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::proto::{DensityFunctionHolder, NoiseParam};
use mcrs_minecraft_worldgen::router::{
    FINAL_DENSITY, NoiseGeneratorSettings, NoiseRouter, RouterBlocks,
};
use mcrs_minecraft_worldgen::volume::Volume;

/// The margin the chunk generator keeps away from zero: f32 interval arithmetic
/// without outward rounding, so a bound landing on zero is not trustworthy.
const SLACK: f32 = 1e-5;

fn router(seed: u64) -> NoiseRouter {
    build(seed, None)
}

/// The overworld with its material rules compiled in, every block and biome
/// they name resolved to a placeholder: the bounds under test are over the
/// vein densities, which read neither.
fn material_router(seed: u64) -> NoiseRouter {
    let rules: BTreeMap<_, MaterialRuleHolder> = corpus::registry("material_rule");
    let conditions: BTreeMap<_, MaterialConditionHolder> = corpus::registry("material_condition");
    build(
        seed,
        Some(&MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &|_| Some(VoxelId(3)),
            biome: &|_| Some(0),
        }),
    )
}

fn build(seed: u64, material: Option<&MaterialInputs<'_>>) -> NoiseRouter {
    let settings: NoiseGeneratorSettings =
        corpus::read("noise_settings", &ResourceLocation::minecraft("overworld"));
    let registry: BTreeMap<_, DensityFunctionHolder> = corpus::registry("density_function");
    let noises: BTreeMap<_, NoiseParam> = corpus::registry("noise");
    build_router(
        &settings,
        &registry,
        &noises,
        seed,
        RouterBlocks {
            default_block: VoxelId(1),
            default_fluid: VoxelId(2),
            water: VoxelId(2),
            lava: VoxelId(3),
        },
        material,
    )
    .unwrap()
}

/// The corner hull of one cell, per lattice row.
fn corner_hull(values: &[f32], stride: usize, size: IVec3, at: IVec3, out: &mut [(f32, f32)]) {
    let index = |x: i32, y: i32, z: i32| (y + (x + z * size.x) * size.y) as usize;
    for (k, bound) in out.iter_mut().enumerate() {
        let row = &values[k * stride..(k + 1) * stride];
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for dz in 0..2 {
            for dx in 0..2 {
                for dy in 0..2 {
                    let v = row[index(at.x + dx, at.y + dy, at.z + dz)];
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
        }
        *bound = (lo, hi);
    }
}

fn lattice_size(cell: IVec3, height: i32) -> IVec3 {
    IVec3::new(16 / cell.x + 1, height / cell.y + 1, 16 / cell.z + 1)
}

#[test]
fn a_settled_cell_bound_contains_every_block_density_in_it() {
    let router = router(845);
    let mut terms = Vec::new();
    let checked = check_root(&router, FINAL_DENSITY, |corners, min, max| {
        router.final_density_cell_bounds(corners, min, max, &mut terms)
    });
    assert!(checked > 0, "no cell produced a bound");
}

/// The surface stage skips the ore vein rule throughout a cell whose density
/// bound stays negative, so the same containment must hold over the vein
/// roots — whose graphs, unlike the terrain's, read `y` above the
/// interpolation.
#[test]
fn a_settled_vein_cell_bound_contains_every_block_density_in_it() {
    let router = material_router(845);
    let veins = router.material().unwrap().veins();
    assert!(!veins.is_empty(), "the overworld ships ore veins");
    for (index, vein) in veins.iter().enumerate() {
        let bounds: &CellBounds = router.vein_cell_bounds(index);
        let mut terms = Vec::new();
        let checked = check_root(&router, vein.density, |corners, min, max| {
            bounds.eval(&router.program, corners, min, max, &mut terms)
        });
        assert!(checked > 0, "vein {index} produced no bound");
    }
}

/// Every cell of a column at the origin whose bound `eval` answers, checked
/// against every block density inside it; returns how many were checked.
fn check_root(
    router: &NoiseRouter,
    root: usize,
    mut eval: impl FnMut(&[Interval], IVec3, IVec3) -> Option<Interval>,
) -> usize {
    let bounds = if root == FINAL_DENSITY {
        None
    } else {
        let veins = router.material().unwrap().veins();
        Some(router.vein_cell_bounds(veins.iter().position(|v| v.density == root).unwrap()))
    };
    let (cell, inputs) = match bounds {
        None => (router.cell_size().unwrap(), router.cell_inputs()),
        Some(bounds) => (bounds.cell_size().unwrap(), bounds.inputs()),
    };
    let height = router.noise.height as i32;
    let size = lattice_size(cell, height);
    let volume = Volume::new(size, IVec3::new(0, router.noise.min_y, 0), cell);
    let mut values = vec![0.0f32; inputs.len() * volume.len()];
    let mut ws = Workspace::new();
    router.fill_nodes(&mut ws, &volume, inputs, &mut values);

    let mut hull = vec![(0.0f32, 0.0f32); inputs.len()];
    let mut corners = vec![Interval::exact(0.0); inputs.len()];
    let mut checked = 0usize;
    for cz in 0..size.z - 1 {
        for cx in 0..size.x - 1 {
            for cy in 0..size.y - 1 {
                let at = IVec3::new(cx, cy, cz);
                corner_hull(&values, volume.len(), size, at, &mut hull);
                for (slot, &(lo, hi)) in corners.iter_mut().zip(hull.iter()) {
                    *slot = Interval::of(lo, hi);
                }
                let origin = IVec3::new(volume.block_x(cx), volume.block_y(cy), volume.block_z(cz));
                let Some(bounds) = eval(&corners, origin, origin + cell - 1) else {
                    continue;
                };
                let dense = Volume::dense(cell, origin);
                let mut density = vec![0.0f32; dense.len()];
                router.program.fill(&mut ws, &dense, root, &mut density);
                for (i, &value) in density.iter().enumerate() {
                    assert!(
                        value >= bounds.min() - SLACK && value <= bounds.max() + SLACK,
                        "cell at {origin:?} index {i}: {value:e} outside [{:e}; {:e}]",
                        bounds.min(),
                        bounds.max()
                    );
                }
                checked += 1;
            }
        }
    }
    checked
}
