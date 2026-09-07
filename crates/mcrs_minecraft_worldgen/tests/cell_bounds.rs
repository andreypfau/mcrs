//! A cell the bound settles is filled with one block state outright, so a bound
//! that does not contain every density inside its cell writes stone through air.

use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_voxel_storage::VoxelId;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::interval::Interval;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::proto::{DensityFunctionHolder, NoiseParam};
use mcrs_minecraft_worldgen::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen::volume::Volume;

/// The margin the chunk generator keeps away from zero: f32 interval arithmetic
/// without outward rounding, so a bound landing on zero is not trustworthy.
const SLACK: f32 = 1e-5;

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn walk_json(base: &Path, dir: &Path, out: &mut Vec<(ResourceLocation, Vec<u8>)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_json(base, &path, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            let rel = path.strip_prefix(base).unwrap();
            let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
            out.push((
                ResourceLocation::parse(&format!("minecraft:{name}")).unwrap(),
                std::fs::read(&path).unwrap(),
            ));
        }
    }
}

fn raw(sub: &str) -> Vec<(ResourceLocation, Vec<u8>)> {
    let dir = assets_dir().join(sub);
    let mut files = Vec::new();
    walk_json(&dir, &dir, &mut files);
    files
}

fn settings_bytes() -> Vec<u8> {
    std::fs::read(assets_dir().join("minecraft/worldgen/noise_settings/overworld.json")).unwrap()
}

fn router(seed: u64) -> NoiseRouter {
    let settings: NoiseGeneratorSettings = serde_json::from_slice(&settings_bytes()).unwrap();
    let registry: BTreeMap<_, DensityFunctionHolder> = raw("minecraft/worldgen/density_function")
        .iter()
        .filter_map(|(id, d)| serde_json::from_slice(d).ok().map(|v| (id.clone(), v)))
        .collect();
    let noises: BTreeMap<_, NoiseParam> = raw("minecraft/worldgen/noise")
        .iter()
        .filter_map(|(id, d)| serde_json::from_slice(d).ok().map(|v| (id.clone(), v)))
        .collect();
    build_router(
        &settings,
        &registry,
        &noises,
        seed,
        VoxelId(1),
        VoxelId(2),
        None,
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
    let cell = router.cell_size().unwrap();
    let height = router.noise_height() as i32;
    let size = lattice_size(cell, height);
    let volume = Volume::new(size, IVec3::new(0, router.noise_min_y(), 0), cell);
    let inputs = router.cell_inputs();
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
                let Some(bounds) = router.final_density_cell_bounds(&corners) else {
                    continue;
                };
                let origin = IVec3::new(volume.block_x(cx), volume.block_y(cy), volume.block_z(cz));
                let dense = Volume::dense(cell, origin);
                let mut density = vec![0.0f32; dense.len()];
                router.fill(&mut ws, &dense, router.final_density(), &mut density);
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
    assert!(checked > 0, "no cell produced a bound");
}
