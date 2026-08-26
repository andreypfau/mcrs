use bevy_math::IVec3;
use mcrs_core::ResourceLocation;
use mcrs_minecraft_worldgen::density_function::proto::{
    DensityFunctionHolder, NoiseParam, ProtoDensityFunction,
};
use mcrs_minecraft_worldgen::density_function::{NoiseRouter, build_functions};
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"MCDFORCL";

struct Volume {
    name: String,
    size: [i32; 3],
    min: [i32; 3],
    step: [i32; 3],
    values: Vec<f32>,
}

struct Dump {
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    volumes: Vec<Volume>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        out
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn i64(&mut self) -> i64 {
        i64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn string(&mut self) -> String {
        let len = self.u32() as usize;
        String::from_utf8(self.take(len).to_vec()).unwrap()
    }
}

fn read_dump(path: &Path) -> Dump {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
    let mut r = Reader {
        data: &data,
        pos: 0,
    };
    assert_eq!(r.take(8), MAGIC, "{} is not an oracle dump", path.display());
    assert_eq!(r.u32(), 1, "unsupported oracle format version");
    let _world_version = r.u32();
    let _settings_id = r.string();
    let seed = r.i64();
    let chunk_x = r.i32();
    let chunk_z = r.i32();
    let volume_count = r.u32() as usize;
    let volumes = (0..volume_count)
        .map(|_| {
            let name = r.string();
            let size = [r.i32(), r.i32(), r.i32()];
            let min = [r.i32(), r.i32(), r.i32()];
            let step = [r.i32(), r.i32(), r.i32()];
            let value_count = r.u32() as usize;
            let values = (0..value_count).map(|_| r.f32()).collect();
            Volume {
                name,
                size,
                min,
                step,
                values,
            }
        })
        .collect();
    assert_eq!(r.pos, data.len(), "trailing bytes in {}", path.display());
    Dump {
        seed,
        chunk_x,
        chunk_z,
        volumes,
    }
}

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vanilla")
}

fn walk_json(base: &Path, dir: &Path, out: &mut Vec<(ResourceLocation, Vec<u8>)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_json(base, &path, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            let rel = path.strip_prefix(base).unwrap();
            let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
            let ident = ResourceLocation::parse(&format!("minecraft:{}", name)).unwrap();
            out.push((ident.into(), std::fs::read(&path).unwrap()));
        }
    }
}

fn resolve_holder(
    id: &ResourceLocation,
    holder: &DensityFunctionHolder,
    all: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
    out: &mut BTreeMap<ResourceLocation, ProtoDensityFunction>,
) {
    if out.contains_key(id) {
        return;
    }
    match holder {
        DensityFunctionHolder::Value(v) => {
            out.insert(id.clone(), ProtoDensityFunction::Constant(v.clone()));
        }
        DensityFunctionHolder::Reference(r) => {
            if let Some(dep) = all.get(r) {
                resolve_holder(id, dep, all, out);
            }
        }
        DensityFunctionHolder::Owned(proto) => {
            out.insert(id.clone(), *proto.clone());
        }
    }
}

fn overworld_router(seed: u64) -> NoiseRouter {
    let assets = assets_dir();
    let settings: NoiseGeneratorSettings = serde_json::from_slice(
        &std::fs::read(assets.join("minecraft/worldgen/noise_settings/overworld.json")).unwrap(),
    )
    .unwrap();

    let df_dir = assets.join("minecraft/worldgen/density_function");
    let mut files = Vec::new();
    walk_json(&df_dir, &df_dir, &mut files);
    let holders: BTreeMap<ResourceLocation, DensityFunctionHolder> = files
        .iter()
        .filter_map(|(id, data)| {
            serde_json::from_slice::<DensityFunctionHolder>(data)
                .ok()
                .map(|h| (id.clone(), h))
        })
        .collect();
    let mut functions = BTreeMap::new();
    for (id, holder) in &holders {
        resolve_holder(id, holder, &holders, &mut functions);
    }

    let noise_dir = assets.join("minecraft/worldgen/noise");
    let mut noise_files = Vec::new();
    walk_json(&noise_dir, &noise_dir, &mut noise_files);
    let noises: BTreeMap<ResourceLocation, NoiseParam> = noise_files
        .iter()
        .filter_map(|(id, data)| {
            serde_json::from_slice::<NoiseParam>(data)
                .ok()
                .map(|n| (id.clone(), n))
        })
        .collect();

    build_functions(
        &functions,
        &noises,
        &settings,
        seed,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    )
}

/// `final_density` at every cell corner of the chunk column, driven through the
/// same calls `generate_column`/`generate_section` make: one
/// `precompute_column_grid` for the whole column, then per-section
/// `fill_plane_cached_reuse` + `on_sampled_cell_corners`.
fn our_final_density_lattice(router: &NoiseRouter, chunk_x: i32, chunk_z: i32) -> Vec<f32> {
    let mut interp = router.new_noise_cell_interpolator();
    let h = interp.h_cell_blocks();
    let v = interp.v_cell_blocks();
    let h_cells = interp.h_cells();
    let v_cells = interp.v_cells();
    let side = h_cells + 1;

    let block_x = chunk_x * 16;
    let block_z = chunk_z * 16;
    let mut cache = router.new_column_cache(block_x, block_z);
    router.populate_columns(&mut cache);

    let noise_min_y = router.noise_min_y();
    let rows = router.noise_height() as usize / v + 1;
    interp.precompute_column_grid(router, &mut cache, noise_min_y, rows);

    let mut out = vec![f32::NAN; side * rows * side];
    let mut put = |xc: usize, row: usize, zc: usize, value: f32| {
        out[row + (xc + zc * side) * rows] = value;
    };

    for sy in (noise_min_y / 16)..((noise_min_y + router.noise_height() as i32) / 16) {
        let base_y = sy * 16;
        interp.fill_plane_cached_reuse(0, true, block_x, base_y, block_z, router, &mut cache);
        for cell_x in 0..h_cells {
            let next_x = block_x + ((cell_x + 1) * h) as i32;
            interp.fill_plane_cached_reuse(
                cell_x + 1,
                false,
                next_x,
                base_y,
                block_z,
                router,
                &mut cache,
            );
            for cell_z in 0..h_cells {
                for cell_y in (0..v_cells).rev() {
                    interp.on_sampled_cell_corners(cell_y, cell_z);
                    let c = *interp.corners();
                    let row = ((base_y - noise_min_y) as usize) / v + cell_y;
                    put(cell_x, row, cell_z, c[0]);
                    put(cell_x, row + 1, cell_z, c[1]);
                    put(cell_x, row, cell_z + 1, c[2]);
                    put(cell_x, row + 1, cell_z + 1, c[3]);
                    put(cell_x + 1, row, cell_z, c[4]);
                    put(cell_x + 1, row + 1, cell_z, c[5]);
                    put(cell_x + 1, row, cell_z + 1, c[6]);
                    put(cell_x + 1, row + 1, cell_z + 1, c[7]);
                }
            }
            interp.swap_buffers();
        }
        interp.end_section();
    }
    assert!(out.iter().all(|v| !v.is_nan()), "corner lattice has holes");
    out
}

fn our_root_lattice(router: &NoiseRouter, root: usize, volume: &Volume) -> Vec<f32> {
    let mut cache = router.new_cache();
    let mut out = Vec::with_capacity(volume.values.len());
    for zi in 0..volume.size[2] {
        for xi in 0..volume.size[0] {
            for yi in 0..volume.size[1] {
                let pos = IVec3::new(
                    volume.min[0] + xi * volume.step[0],
                    volume.min[1] + yi * volume.step[1],
                    volume.min[2] + zi * volume.step[2],
                );
                out.push(router.sample_root(root, pos, &mut cache));
            }
        }
    }
    out
}

fn ordered(x: f32) -> i64 {
    let bits = x.to_bits() as i64;
    if x.is_sign_negative() {
        -(bits & 0x7fff_ffff)
    } else {
        bits
    }
}

const COARSE_TOLERANCE: f32 = 1e-3;

struct Diff {
    mismatches: usize,
    coarse: usize,
    max_abs: f32,
    max_ulp: i64,
    worst: IVec3,
    worst_pair: (f32, f32),
}

fn compare(volume: &Volume, ours: &[f32]) -> Diff {
    assert_eq!(ours.len(), volume.values.len());
    let mut d = Diff {
        mismatches: 0,
        coarse: 0,
        max_abs: 0.0,
        max_ulp: 0,
        worst: IVec3::ZERO,
        worst_pair: (0.0, 0.0),
    };
    for zi in 0..volume.size[2] {
        for xi in 0..volume.size[0] {
            for yi in 0..volume.size[1] {
                let i = (yi + (xi + zi * volume.size[0]) * volume.size[1]) as usize;
                let (want, got) = (volume.values[i], ours[i]);
                if want.to_bits() == got.to_bits() {
                    continue;
                }
                d.mismatches += 1;
                let abs = (want - got).abs();
                if abs > COARSE_TOLERANCE {
                    d.coarse += 1;
                }
                let ulp = (ordered(want) - ordered(got)).abs();
                if abs > d.max_abs {
                    d.max_abs = abs;
                    d.worst = IVec3::new(
                        volume.min[0] + xi * volume.step[0],
                        volume.min[1] + yi * volume.step[1],
                        volume.min[2] + zi * volume.step[2],
                    );
                    d.worst_pair = (want, got);
                }
                d.max_ulp = d.max_ulp.max(ulp);
            }
        }
    }
    d
}

const LATTICE_FILES: &[&str] = &[
    "overworld_s1_c0_0.bin",
    "overworld_s1_c10_-7.bin",
    "overworld_s1_c100_100.bin",
    "overworld_s1_c-33_55.bin",
    "overworld_s1_c7_7.bin",
    "overworld_s2_c0_0.bin",
    "overworld_s2_c10_-7.bin",
    "overworld_s2_c100_100.bin",
    "overworld_s2_c-33_55.bin",
    "overworld_s2_c7_7.bin",
    "overworld_s42_c0_0.bin",
    "overworld_s42_c10_-7.bin",
    "overworld_s42_c100_100.bin",
    "overworld_s42_c-33_55.bin",
    "overworld_s42_c7_7.bin",
];

/// Per-root diff totals across every corner-lattice dump.
fn lattice_report() -> BTreeMap<String, Diff> {
    let mut report: BTreeMap<String, Diff> = BTreeMap::new();
    for file in LATTICE_FILES {
        let dump = read_dump(&fixtures_dir().join(file));
        let router = overworld_router(dump.seed as u64);
        let roots: BTreeMap<&str, usize> = router.roots().into_iter().collect();

        for volume in &dump.volumes {
            assert_eq!(volume.min[0], dump.chunk_x * 16);
            assert_eq!(volume.min[2], dump.chunk_z * 16);
            let ours = if volume.name == "final_density" {
                our_final_density_lattice(&router, dump.chunk_x, dump.chunk_z)
            } else {
                our_root_lattice(&router, roots[volume.name.as_str()], volume)
            };
            let d = compare(volume, &ours);
            let slot = report.entry(volume.name.clone()).or_insert(Diff {
                mismatches: 0,
                coarse: 0,
                max_abs: 0.0,
                max_ulp: 0,
                worst: IVec3::ZERO,
                worst_pair: (0.0, 0.0),
            });
            slot.mismatches += d.mismatches;
            slot.coarse += d.coarse;
            slot.max_ulp = slot.max_ulp.max(d.max_ulp);
            if d.max_abs > slot.max_abs {
                slot.max_abs = d.max_abs;
                slot.worst = d.worst;
                slot.worst_pair = d.worst_pair;
            }
        }
    }
    report
}

fn print_report(report: &BTreeMap<String, Diff>, total: usize) {
    for (name, d) in report {
        println!(
            "{:>20}: {:>6}/{} differ, {:>6} by more than {:e}  max_abs={:e}  max_ulp={}  worst@{:?} vanilla={} ours={}",
            name,
            d.mismatches,
            total,
            d.coarse,
            COARSE_TOLERANCE,
            d.max_abs,
            d.max_ulp,
            d.worst,
            d.worst_pair.0,
            d.worst_pair.1
        );
    }
}

/// Vanilla evaluates every noise in double precision and narrows to float only
/// when storing into `DensityBuffer`; we evaluate the whole tree in f32. No root
/// is therefore bit-identical. These roots carry nothing but that drift.
const F32_DRIFT_ROOTS: &[&str] = &[
    "continents",
    "depth",
    "erosion",
    "ridges",
    "temperature",
    "vegetation",
];

/// Largest observed f32-versus-f64 drift on `F32_DRIFT_ROOTS` is 3.7e-5.
const DRIFT_TOLERANCE: f32 = 1e-4;

#[test]
fn climate_and_depth_roots_track_the_vanilla_oracle_within_f32_drift() {
    let report = lattice_report();
    let total = LATTICE_FILES.len() * 1225;
    print_report(&report, total);
    for name in F32_DRIFT_ROOTS {
        let d = &report[*name];
        assert!(
            d.max_abs < DRIFT_TOLERANCE,
            "{}: max_abs={:e} exceeds {:e} (max_ulp={}, worst@{:?} vanilla={} ours={})",
            name,
            d.max_abs,
            DRIFT_TOLERANCE,
            d.max_ulp,
            d.worst,
            d.worst_pair.0,
            d.worst_pair.1
        );
        assert_eq!(
            d.coarse, 0,
            "{}: {} values differ by more than {:e}",
            name, d.coarse, COARSE_TOLERANCE
        );
    }
}

/// Bit-exact parity for all eight roots. Fails today on every root: the six
/// above by f32 drift only, `chunk_surface_level` and `final_density` by more.
#[test]
#[ignore = "known divergence from vanilla; run to measure it"]
fn all_roots_match_the_vanilla_oracle_bit_for_bit() {
    let report = lattice_report();
    let total = LATTICE_FILES.len() * 1225;
    print_report(&report, total);
    for (name, d) in &report {
        assert_eq!(
            d.mismatches,
            0,
            "{}: {} of {} values differ from vanilla (max_abs={:e}, max_ulp={}, worst@{:?} vanilla={} ours={})",
            name,
            d.mismatches,
            total,
            d.max_abs,
            d.max_ulp,
            d.worst,
            d.worst_pair.0,
            d.worst_pair.1
        );
    }
}

/// Per-block `final_density` over a whole chunk, through the trilinear
/// interpolation `generate_section` runs. This discriminates interpolation
/// scope: vanilla lerps only the `interpolated` sub-tree and applies squeeze,
/// min-with-noodle and add-beardifier per block outside it, so a whole-root
/// interpolation disagrees here even where the corner lattice agrees.
#[test]
#[ignore = "known divergence from vanilla; run to measure it"]
fn dense_final_density_matches_the_vanilla_oracle() {
    let dump = read_dump(&fixtures_dir().join("overworld_s42_c0_0_dense.bin"));
    let router = overworld_router(dump.seed as u64);
    let volume = &dump.volumes[0];

    let mut interp = router.new_noise_cell_interpolator();
    let h = interp.h_cell_blocks();
    let v = interp.v_cell_blocks();
    let h_cells = interp.h_cells();
    let v_cells = interp.v_cells();

    let block_x = dump.chunk_x * 16;
    let block_z = dump.chunk_z * 16;
    let mut cache = router.new_column_cache(block_x, block_z);
    router.populate_columns(&mut cache);
    let noise_min_y = router.noise_min_y();
    let rows = router.noise_height() as usize / v + 1;
    interp.precompute_column_grid(&router, &mut cache, noise_min_y, rows);

    let mut ours = vec![f32::NAN; volume.values.len()];
    for sy in (noise_min_y / 16)..((noise_min_y + router.noise_height() as i32) / 16) {
        let base_y = sy * 16;
        interp.fill_plane_cached_reuse(0, true, block_x, base_y, block_z, &router, &mut cache);
        for cell_x in 0..h_cells {
            let next_x = block_x + ((cell_x + 1) * h) as i32;
            interp.fill_plane_cached_reuse(
                cell_x + 1,
                false,
                next_x,
                base_y,
                block_z,
                &router,
                &mut cache,
            );
            for cell_z in 0..h_cells {
                for cell_y in (0..v_cells).rev() {
                    interp.on_sampled_cell_corners(cell_y, cell_z);
                    for local_y in (0..v).rev() {
                        interp.interpolate_y(local_y as f32 / v as f32);
                        let world_y = base_y + (cell_y * v + local_y) as i32;
                        for local_x in 0..h {
                            interp.interpolate_x(local_x as f32 / h as f32);
                            for local_z in 0..h {
                                interp.interpolate_z(local_z as f32 / h as f32);
                                let xi = (cell_x * h + local_x) as i32;
                                let zi = (cell_z * h + local_z) as i32;
                                let yi = world_y - volume.min[1];
                                let i = (yi + (xi + zi * volume.size[0]) * volume.size[1]) as usize;
                                ours[i] = interp.result();
                            }
                        }
                    }
                }
            }
            interp.swap_buffers();
        }
        interp.end_section();
    }
    assert!(ours.iter().all(|v| !v.is_nan()), "dense volume has holes");

    let d = compare(volume, &ours);
    assert_eq!(
        d.mismatches,
        0,
        "dense final_density: {} of {} values differ (max_abs={:e}, max_ulp={}, worst@{:?} vanilla={} ours={})",
        d.mismatches,
        volume.values.len(),
        d.max_abs,
        d.max_ulp,
        d.worst,
        d.worst_pair.0,
        d.worst_pair.1
    );
}
