use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
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
            out.insert(id.clone(), ProtoDensityFunction::Constant(*v));
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
    router_from_settings(overworld_settings(), seed)
}

fn overworld_settings() -> serde_json::Value {
    serde_json::from_slice(
        &std::fs::read(assets_dir().join("minecraft/worldgen/noise_settings/overworld.json"))
            .unwrap(),
    )
    .unwrap()
}

fn router_from_settings(settings: serde_json::Value, seed: u64) -> NoiseRouter {
    let assets = assets_dir();
    let settings: NoiseGeneratorSettings = serde_json::from_value(settings).unwrap();

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

/// `final_density` over a dump volume, through the same `fill` the chunk
/// generator drives.
fn our_fill(router: &NoiseRouter, root: usize, volume: &Volume) -> Vec<f32> {
    let box_volume = mcrs_minecraft_worldgen::density_function::Volume::new(
        IVec3::from_array(volume.size),
        IVec3::from_array(volume.min),
        IVec3::from_array(volume.step),
    );
    let mut out = vec![f32::NAN; box_volume.len()];
    let mut scratch = mcrs_minecraft_worldgen::density_function::FillScratch::new();
    router.fill(root, &box_volume, &mut out, &mut scratch);
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
                our_fill(&router, router.final_density_index(), volume)
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

/// Vanilla evaluates the noise tree in double and narrows to float only when
/// storing into `DensityBuffer`; we evaluate in f32 throughout. Every root but
/// `final_density` still lands bit-exact, so only that one carries a tolerance:
/// 2.98e-8 observed, two f32 ulps at the magnitude where it occurs. A budget
/// that small cannot mask a structural divergence — the 1e-3 counter asserted
/// alongside it stays at zero.
const FINAL_DENSITY_DRIFT: f32 = 4e-8;

#[test]
fn all_roots_match_the_vanilla_oracle() {
    let report = lattice_report();
    let total = LATTICE_FILES.len() * 1225;
    print_report(&report, total);
    for (name, d) in &report {
        assert_eq!(
            d.coarse, 0,
            "{}: {} values differ by more than {:e} (worst@{:?} vanilla={} ours={})",
            name, d.coarse, COARSE_TOLERANCE, d.worst, d.worst_pair.0, d.worst_pair.1
        );
        if name == "final_density" {
            assert!(
                d.max_abs < FINAL_DENSITY_DRIFT,
                "final_density: max_abs={:e} exceeds {:e} ({} of {} differ, max_ulp={}, worst@{:?} vanilla={} ours={})",
                d.max_abs,
                FINAL_DENSITY_DRIFT,
                d.mismatches,
                total,
                d.max_ulp,
                d.worst,
                d.worst_pair.0,
                d.worst_pair.1
            );
            continue;
        }
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

/// Per-block `final_density` over a whole chunk, through the same `fill` the
/// chunk generator drives — both as one box and cell by cell, which is the
/// shape the generator actually uses once interval arithmetic leaves a cell
/// undecided. Vanilla's own per-block path is `fillCell`, so this lands
/// bit-exact rather than merely close.
#[test]
fn dense_final_density_matches_the_vanilla_oracle() {
    let dump = read_dump(&fixtures_dir().join("overworld_s42_c0_0_dense.bin"));
    let router = overworld_router(dump.seed as u64);
    let volume = &dump.volumes[0];
    assert_eq!(volume.step, [1, 1, 1]);

    let whole = our_fill(&router, router.final_density_index(), volume);
    let d = compare(volume, &whole);
    println!(
        "dense fill vs oracle: {}/{} differ  max_abs={:e}  max_ulp={}  worst@{:?} vanilla={} ours={}",
        d.mismatches,
        volume.values.len(),
        d.max_abs,
        d.max_ulp,
        d.worst,
        d.worst_pair.0,
        d.worst_pair.1
    );
    assert_eq!(
        d.mismatches,
        0,
        "dense fill: {} of {} values differ from vanilla (max_abs={:e}, max_ulp={}, worst@{:?} vanilla={} ours={})",
        d.mismatches,
        volume.values.len(),
        d.max_abs,
        d.max_ulp,
        d.worst,
        d.worst_pair.0,
        d.worst_pair.1
    );

    let cell = router
        .cell_size()
        .expect("the overworld has one cell geometry");
    let mut by_cell = vec![f32::NAN; whole.len()];
    let mut scratch = mcrs_minecraft_worldgen::density_function::FillScratch::new();
    let mut cell_values = vec![0.0f32; (cell.x * cell.y * cell.z) as usize];
    for z in (0..volume.size[2]).step_by(cell.z as usize) {
        for x in (0..volume.size[0]).step_by(cell.x as usize) {
            for y in (0..volume.size[1]).step_by(cell.y as usize) {
                let cell_volume = mcrs_minecraft_worldgen::density_function::Volume::dense(
                    cell,
                    IVec3::new(volume.min[0] + x, volume.min[1] + y, volume.min[2] + z),
                );
                router.fill(
                    router.final_density_index(),
                    &cell_volume,
                    &mut cell_values,
                    &mut scratch,
                );
                for (i, &value) in cell_values.iter().enumerate() {
                    let (cx, cy, cz) = (
                        (i as i32 / cell.y) % cell.x,
                        i as i32 % cell.y,
                        i as i32 / (cell.y * cell.x),
                    );
                    let j =
                        (y + cy + (x + cx + (z + cz) * volume.size[0]) * volume.size[1]) as usize;
                    by_cell[j] = value;
                }
            }
        }
    }
    let mismatched = whole
        .iter()
        .zip(&by_cell)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    assert_eq!(
        mismatched,
        0,
        "{mismatched} of {} values move when the same volume is filled cell by cell",
        whole.len()
    );
}

/// `final_density_cell_bounds` drives the whole-cell fill fast path, so a bound
/// that is ever tighter than the block values inside the cell would place solid
/// blocks in air. Interval arithmetic in f32 is not outward-rounded, so this
/// also measures how far the bound can sit on the wrong side.
#[test]
fn cell_bounds_contain_every_block_density() {
    let dump = read_dump(&fixtures_dir().join("overworld_s42_c0_0_dense.bin"));
    let router = overworld_router(dump.seed as u64);
    let volume = &dump.volumes[0];
    let cell = router
        .cell_size()
        .expect("the overworld has one cell geometry");
    let block_min = IVec3::from_array(volume.min);
    let cells = IVec3::from_array(volume.size) / cell;

    let roots = router.cell_value_roots();
    let width = roots.len();
    let corner_volume =
        mcrs_minecraft_worldgen::density_function::Volume::new(cells + IVec3::ONE, block_min, cell);
    let mut scratch = mcrs_minecraft_worldgen::density_function::FillScratch::new();
    let mut corners = vec![0.0f32; width * corner_volume.len()];
    router.fill_roots(roots, &corner_volume, &mut corners, &mut scratch);

    let mut bounds_scratch = vec![(0.0f32, 0.0f32); router.final_density_index() + 1];
    let mut cell_values = vec![0.0f32; (cell.x * cell.y * cell.z) as usize];
    let mut worst_low = 0.0f32;
    let mut worst_high = 0.0f32;
    let mut bounded_cells = 0usize;
    let mut total_cells = 0usize;

    for cz in 0..cells.z {
        for cx in 0..cells.x {
            for cy in 0..cells.y {
                total_cells += 1;
                let wrapper_bounds: Vec<(f32, f32)> = (0..width)
                    .map(|k| {
                        let row = &corners[k * corner_volume.len()..(k + 1) * corner_volume.len()];
                        let mut lo = f32::INFINITY;
                        let mut hi = f32::NEG_INFINITY;
                        for dz in 0..2 {
                            for dx in 0..2 {
                                for dy in 0..2 {
                                    let v = row
                                        [corner_volume.index_unchecked(cx + dx, cy + dy, cz + dz)];
                                    lo = lo.min(v);
                                    hi = hi.max(v);
                                }
                            }
                        }
                        (lo, hi)
                    })
                    .collect();
                let Some((lo, hi)) =
                    router.final_density_cell_bounds(&wrapper_bounds, &mut bounds_scratch)
                else {
                    continue;
                };
                bounded_cells += 1;
                let cell_volume = mcrs_minecraft_worldgen::density_function::Volume::dense(
                    cell,
                    block_min + IVec3::new(cx, cy, cz) * cell,
                );
                router.fill(
                    router.final_density_index(),
                    &cell_volume,
                    &mut cell_values,
                    &mut scratch,
                );
                for &value in &cell_values {
                    worst_low = worst_low.max(lo - value);
                    worst_high = worst_high.max(value - hi);
                }
            }
        }
    }

    println!(
        "cell bounds: {}/{} cells bounded, worst overshoot low={:e} high={:e}",
        bounded_cells, total_cells, worst_low, worst_high
    );
    assert_eq!(
        bounded_cells, total_cells,
        "overworld final_density should be fully interval-bounded"
    );
    assert!(
        worst_low < CELL_BOUNDS_SLACK && worst_high < CELL_BOUNDS_SLACK,
        "cell bounds violated by more than {:e}: low={:e} high={:e}",
        CELL_BOUNDS_SLACK,
        worst_low,
        worst_high
    );
}

/// Margin the fill fast path keeps away from zero, covering the rounding f32
/// interval arithmetic accumulates over the outer terms.
const CELL_BOUNDS_SLACK: f32 = 1e-5;

/// Both Zone B evaluation paths carry a debug-only check that the branch
/// schedule's skipped runs never move a value the caller reads back. Neither
/// parity test above reaches the scalar path, and skips are position-dependent,
/// so this sweeps whole columns block by block to drive the check.
#[test]
fn branch_skipping_preserves_every_zone_b_root() {
    for seed in [1u64, 42] {
        let router = overworld_router(seed);
        for (cx, cz) in [(0i32, 0i32), (-33, 55), (7, 7), (100, -7)] {
            let (bx, bz) = (cx * 16, cz * 16);
            let mut cache = router.new_column_cache(bx, bz);
            router.populate_columns(&mut cache);
            let min_y = router.noise_min_y();
            let height = router.noise_height() as i32;

            for lx in (0..=16).step_by(4) {
                for lz in (0..=16).step_by(4) {
                    cache.load_column(lx, lz);
                    for y in min_y..min_y + height {
                        router.final_density_from_column_cache(
                            IVec3::new(bx + lx, y, bz + lz),
                            &mut cache,
                        );
                    }
                }
            }
        }
    }
}

struct FillDiff {
    mismatches: usize,
    total: usize,
    max_ulp: i64,
    max_abs: f32,
    worst: IVec3,
}

fn fill_vs_sample_root(
    router: &NoiseRouter,
    root: usize,
    volume: &mcrs_minecraft_worldgen::density_function::Volume,
) -> FillDiff {
    let mut scratch = mcrs_minecraft_worldgen::density_function::FillScratch::new();
    let mut filled = vec![f32::NAN; volume.len()];
    router.fill(root, volume, &mut filled, &mut scratch);

    let mut cache = router.new_cache();
    let mut d = FillDiff {
        mismatches: 0,
        total: volume.len(),
        max_ulp: 0,
        max_abs: 0.0,
        worst: IVec3::ZERO,
    };
    for z in 0..volume.size_z() {
        for x in 0..volume.size_x() {
            for y in 0..volume.size_y() {
                let pos = IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                let want = router.sample_root(root, pos, &mut cache);
                let got = filled[volume.index_unchecked(x, y, z)];
                if want.to_bits() == got.to_bits() {
                    continue;
                }
                d.mismatches += 1;
                d.max_ulp = d.max_ulp.max((ordered(want) - ordered(got)).abs());
                let abs = (want - got).abs();
                if abs > d.max_abs {
                    d.max_abs = abs;
                    d.worst = pos;
                }
            }
        }
    }
    d
}

/// Volumes that already sample the router's cell lattice. Every `interpolated`
/// wrapper passes its input straight through there, so the volume fill and the
/// scalar sample run identical arithmetic.
fn lattice_fill_cases(
    router: &NoiseRouter,
) -> Vec<(
    &'static str,
    mcrs_minecraft_worldgen::density_function::Volume,
)> {
    use mcrs_minecraft_worldgen::density_function::Volume as V;
    let step = router
        .cell_size()
        .expect("the overworld has one cell geometry");
    vec![
        (
            "cell lattice",
            V::new(IVec3::new(5, 5, 5), IVec3::new(0, -64, 0), step),
        ),
        (
            "2x2x2 cell corners",
            V::new(IVec3::new(2, 2, 2), IVec3::new(8, -8, 8), step),
        ),
        (
            "negative cell corners",
            V::new(IVec3::new(3, 4, 3), IVec3::new(-36, -64, -52), step),
        ),
        (
            "single cell corner",
            V::new(IVec3::ONE, IVec3::new(16, 32, -48), step),
        ),
    ]
}

/// Volumes that cut across cells, where the two paths deliberately diverge.
fn interior_fill_cases() -> Vec<(
    &'static str,
    mcrs_minecraft_worldgen::density_function::Volume,
)> {
    use mcrs_minecraft_worldgen::density_function::Volume as V;
    vec![
        (
            "dense box over several cells",
            V::dense(IVec3::new(8, 9, 8), IVec3::new(4, -16, 4)),
        ),
        (
            "single column",
            V::dense(IVec3::new(1, 32, 1), IVec3::new(13, -32, -27)),
        ),
        (
            "16x1x16 plane",
            V::dense(IVec3::new(16, 1, 16), IVec3::new(0, 63, 0)),
        ),
        (
            "unaligned min",
            V::dense(IVec3::new(3, 3, 3), IVec3::new(5, -59, 7)),
        ),
        (
            "negative min",
            V::dense(IVec3::new(4, 4, 4), IVec3::new(-37, -64, -53)),
        ),
        (
            "unaligned stride",
            V::new(
                IVec3::new(4, 4, 4),
                IVec3::new(2, -62, 6),
                IVec3::new(2, 3, 2),
            ),
        ),
    ]
}

#[test]
fn fill_matches_sample_root_on_the_cell_lattice() {
    let router = overworld_router(42);
    let mut roots: Vec<(&str, usize)> = router.roots();
    roots.push(("final_density", router.final_density_index()));
    for (root_name, root) in roots {
        for (name, volume) in lattice_fill_cases(&router) {
            let d = fill_vs_sample_root(&router, root, &volume);
            assert_eq!(
                d.mismatches, 0,
                "{name} / {root_name}: {} of {} values differ (max_ulp={}, max_abs={:e}, worst@{:?})",
                d.mismatches, d.total, d.max_ulp, d.max_abs, d.worst
            );
        }
    }
}

/// Off the cell lattice the two paths are vanilla's two paths, and vanilla's
/// own arithmetic differs between them: `sampleValue` combines the eight cell
/// corners with an exact `lerp3`, while `fillCell` accumulates along Y. The
/// gap is one f32 ulp of the wrapper value, carried through the per-block terms
/// above the wrapper: 5.96e-8 observed, one ulp at unit magnitude.
const FILL_VERSUS_SCALAR: f32 = 8e-8;

#[test]
fn fill_and_sample_root_differ_only_by_the_y_accumulation() {
    let router = overworld_router(42);
    let mut roots: Vec<(&str, usize)> = router.roots();
    roots.push(("final_density", router.final_density_index()));
    for (root_name, root) in roots {
        for (name, volume) in interior_fill_cases() {
            let d = fill_vs_sample_root(&router, root, &volume);
            println!(
                "{:>20} / {:<28}: {:>5}/{} differ  max_ulp={}  max_abs={:e}  worst@{:?}",
                root_name, name, d.mismatches, d.total, d.max_ulp, d.max_abs, d.worst
            );
            assert!(
                d.max_abs < FILL_VERSUS_SCALAR,
                "{name} / {root_name}: max_abs={:e} exceeds {:e} (max_ulp={}, worst@{:?})",
                d.max_abs,
                FILL_VERSUS_SCALAR,
                d.max_ulp,
                d.worst
            );
        }
    }
}

/// Vanilla puts no single cell geometry on a router, and the shipped corpus
/// already carries three. A `final_density` reading two at once must load and
/// evaluate, with each wrapper interpolating on its own lattice.
#[test]
fn a_router_mixing_cell_geometries_loads_and_evaluates() {
    let mut settings = overworld_settings();
    let coarse = serde_json::json!({
        "type": "minecraft:interpolated",
        "cell_size_xz": 8,
        "cell_size_y": 4,
        "input": {
            "type": "minecraft:mul",
            "left": "minecraft:overworld/depth",
            "right": 0.25
        }
    });
    settings["noise_router"]["final_density"] = serde_json::json!({
        "type": "minecraft:add",
        "left": "minecraft:overworld/final_density",
        "right": coarse
    });
    let router = router_from_settings(settings, 42);
    let plain = overworld_router(42);

    let volume = mcrs_minecraft_worldgen::density_function::Volume::dense(
        IVec3::new(9, 17, 9),
        IVec3::new(-4, -20, 12),
    );
    let d = fill_vs_sample_root(&router, router.final_density_index(), &volume);
    println!(
        "mixed geometry: {}/{} differ  max_ulp={}  max_abs={:e}  worst@{:?}",
        d.mismatches, d.total, d.max_ulp, d.max_abs, d.worst
    );
    assert!(
        d.max_abs < FILL_VERSUS_SCALAR,
        "mixed geometry: max_abs={:e} exceeds {:e} (max_ulp={}, worst@{:?})",
        d.max_abs,
        FILL_VERSUS_SCALAR,
        d.max_ulp,
        d.worst
    );

    let mut cache = router.new_cache();
    let mut plain_cache = plain.new_cache();
    let mut moved = 0usize;
    for z in 0..volume.size_z() {
        for x in 0..volume.size_x() {
            for y in 0..volume.size_y() {
                let pos = IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                let with = router.sample_root(router.final_density_index(), pos, &mut cache);
                let without = plain.sample_root(plain.final_density_index(), pos, &mut plain_cache);
                assert!(with.is_finite(), "non-finite density at {pos:?}");
                if with != without {
                    moved += 1;
                }
            }
        }
    }
    assert!(
        moved > volume.len() / 2,
        "the second wrapper moved only {moved} of {} values",
        volume.len()
    );
}

