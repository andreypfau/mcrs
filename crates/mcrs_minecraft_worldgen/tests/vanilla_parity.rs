use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::proto::{DensityFunctionHolder, NoiseParam};
use mcrs_minecraft_worldgen::router::{NoiseGeneratorSettings, NoiseRouter, ROOT_NAMES};
use mcrs_minecraft_worldgen::volume::Volume;
use mcrs_voxel_storage::VoxelId;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"MCDFORCL";
/// The strict profile is the oracle and must reproduce the dumps bit for bit.
/// The fast profile reassociates and fuses roundings by design, so it is held to
/// a divergence budget instead — the divergence is measured, not waived. The
/// observed worst is 3.3e-6 on `final_density`; a break in the function itself
/// would be orders of magnitude wider.
#[cfg(not(any(feature = "fast_fma", feature = "fast_cell", feature = "fast_ramp")))]
const DIVERGENCE_BUDGET: Option<f32> = None;
#[cfg(any(feature = "fast_fma", feature = "fast_cell", feature = "fast_ramp"))]
const DIVERGENCE_BUDGET: Option<f32> = Some(1.0e-5);
/// `SharedConstants.WORLD_VERSION` of the snapshot the dumps came from. Asserted
/// rather than skipped, so a corpus bump cannot silently invalidate the oracle.
const WORLD_VERSION: u32 = 5015;

struct DumpVolume {
    name: String,
    size: [i32; 3],
    min: [i32; 3],
    step: [i32; 3],
    values: Vec<f32>,
}

struct Dump {
    seed: i64,
    volumes: Vec<DumpVolume>,
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
    assert_eq!(
        r.u32(),
        WORLD_VERSION,
        "{} was dumped from a different snapshot than this corpus targets",
        path.display()
    );
    let _settings_id = r.string();
    let seed = r.i64();
    let _chunk_x = r.i32();
    let _chunk_z = r.i32();
    let volume_count = r.u32() as usize;
    let volumes = (0..volume_count)
        .map(|_| {
            let name = r.string();
            let size = [r.i32(), r.i32(), r.i32()];
            let min = [r.i32(), r.i32(), r.i32()];
            let step = [r.i32(), r.i32(), r.i32()];
            let value_count = r.u32() as usize;
            let values = (0..value_count).map(|_| r.f32()).collect();
            DumpVolume {
                name,
                size,
                min,
                step,
                values,
            }
        })
        .collect();
    assert_eq!(r.pos, data.len(), "trailing bytes in {}", path.display());
    Dump { seed, volumes }
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
            let ident = ResourceLocation::parse(&format!("minecraft:{name}")).unwrap();
            out.push((ident, std::fs::read(&path).unwrap()));
        }
    }
}

fn load<T: serde::de::DeserializeOwned>(sub: &str) -> BTreeMap<ResourceLocation, T> {
    let dir = assets_dir().join(sub);
    let mut files = Vec::new();
    walk_json(&dir, &dir, &mut files);
    files
        .iter()
        .filter_map(|(id, data)| {
            serde_json::from_slice::<T>(data)
                .ok()
                .map(|v| (id.clone(), v))
        })
        .collect()
}

fn overworld_router(seed: u64) -> NoiseRouter {
    let settings: NoiseGeneratorSettings = serde_json::from_slice(
        &std::fs::read(assets_dir().join("minecraft/worldgen/noise_settings/overworld.json"))
            .unwrap(),
    )
    .unwrap();
    let registry: BTreeMap<ResourceLocation, DensityFunctionHolder> =
        load("minecraft/worldgen/density_function");
    let noises: BTreeMap<ResourceLocation, NoiseParam> = load("minecraft/worldgen/noise");
    build_router(&settings, &registry, &noises, seed, VoxelId(1), VoxelId(2))
        .expect("overworld router")
}

fn fill(router: &NoiseRouter, root: usize, v: &DumpVolume) -> Vec<f32> {
    let volume = Volume::new(
        IVec3::from_array(v.size),
        IVec3::from_array(v.min),
        IVec3::from_array(v.step),
    );
    let mut out = vec![f32::NAN; volume.len()];
    let mut ws = Workspace::new();
    router.program().fill(&mut ws, &volume, root, &mut out);
    out
}

#[derive(Default)]
struct Diff {
    checked: usize,
    mismatched: usize,
    worst: f32,
    example: Option<(usize, f32, f32)>,
}

/// Every root the compiler supports, over all fifteen lattice dumps.
#[test]
fn supported_roots_match_the_vanilla_oracle() {
    let mut dumps: Vec<PathBuf> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "bin"))
        .filter(|p| !p.to_string_lossy().contains("dense"))
        .collect();
    dumps.sort();
    assert_eq!(dumps.len(), 15, "expected the fifteen lattice dumps");

    let mut report: BTreeMap<String, Diff> = BTreeMap::new();
    let mut unsupported: Vec<String> = Vec::new();
    let mut seen_names: Vec<String> = Vec::new();

    for path in &dumps {
        let dump = read_dump(path);
        let router = overworld_router(dump.seed as u64);
        if unsupported.is_empty() {
            unsupported = router
                .failed_roots()
                .iter()
                .map(|(name, err)| format!("{name}: {err}"))
                .collect();
        }
        let failed: Vec<&str> = router.failed_roots().iter().map(|(n, _)| *n).collect();

        for v in &dump.volumes {
            if !seen_names.contains(&v.name) {
                seen_names.push(v.name.clone());
            }
            let Some(root) = ROOT_NAMES.iter().position(|n| *n == v.name) else {
                continue;
            };
            if failed.contains(&ROOT_NAMES[root]) {
                continue;
            }
            let ours = fill(&router, root, v);
            assert_eq!(ours.len(), v.values.len(), "{} size", v.name);
            let slot = report.entry(v.name.clone()).or_default();
            for (i, (&got, &want)) in ours.iter().zip(&v.values).enumerate() {
                slot.checked += 1;
                if got.to_bits() != want.to_bits() {
                    let d = (got - want).abs();
                    if !(d <= slot.worst) {
                        slot.worst = d;
                        slot.example = Some((i, got, want));
                    }
                    slot.mismatched += 1;
                }
            }
        }
    }

    let mut lines = Vec::new();
    lines.push(format!("dump volume names: {seen_names:?}"));
    lines.push(format!("roots not yet compilable: {unsupported:?}"));
    let mut bad = false;
    for (name, d) in &report {
        lines.push(format!(
            "{name}: {}/{} mismatched, worst abs diff {:e}{}",
            d.mismatched,
            d.checked,
            d.worst,
            match d.example {
                Some((i, got, want)) => format!(" (index {i}: got {got:e}, want {want:e})"),
                None => String::new(),
            }
        ));
        let within = match DIVERGENCE_BUDGET {
            None => d.mismatched == 0,
            Some(budget) => d.worst <= budget,
        };
        if !within {
            bad = true;
        }
    }
    println!("{}", lines.join("\n"));
    assert!(
        !report.is_empty(),
        "no root was compared\n{}",
        lines.join("\n")
    );
    assert!(!bad, "{}", lines.join("\n"));
}

/// The dense dump is the only fixture that reaches the off-lattice interpolation
/// path: on the lattice dumps the interpolated values are surface levels, exact
/// multiples of eight, so no rounding happens and the interpolation order cannot
/// be distinguished.
#[test]
fn final_density_matches_the_dense_oracle() {
    let path = fixtures_dir().join("overworld_s42_c0_0_dense.bin");
    let dump = read_dump(&path);
    let router = overworld_router(dump.seed as u64);
    assert!(
        router.failed_roots().is_empty(),
        "{:?}",
        router.failed_roots()
    );

    let v = dump
        .volumes
        .iter()
        .find(|v| v.name == "final_density")
        .expect("dense dump holds final_density");
    assert_eq!(v.values.len(), 98_304, "16x384x16 dense");

    let ours = fill(&router, router.final_density(), v);
    let mut mismatched = 0usize;
    let mut worst = 0.0f32;
    let mut example = None;
    for (i, (&got, &want)) in ours.iter().zip(&v.values).enumerate() {
        if got.to_bits() != want.to_bits() {
            mismatched += 1;
            let d = (got - want).abs();
            if !(d <= worst) {
                worst = d;
                example = Some((i, got, want));
            }
        }
    }
    println!(
        "final_density dense: {mismatched}/{} mismatched, worst abs diff {worst:e}",
        v.values.len()
    );
    let within = match DIVERGENCE_BUDGET {
        None => mismatched == 0,
        Some(budget) => worst <= budget,
    };
    assert!(
        within,
        "{mismatched}/{} mismatched, worst {worst:e} {example:?}",
        v.values.len()
    );
}

/// The schedule decides per fill, not per column: an arm's cone is skipped only
/// when no position in the whole volume selects it. Over a box as large as a
/// chunk that is a weaker predicate than a per-column one, so the share it skips
/// is asserted rather than trusted.
#[test]
fn the_branch_schedule_skips_a_real_share_of_an_overworld_chunk() {
    let router = overworld_router(42);
    let volume = Volume::dense(IVec3::new(16, 384, 16), IVec3::new(0, -64, 0));
    let mut out = vec![f32::NAN; volume.len()];
    let mut ws = Workspace::new();
    ws.take_count();
    router
        .program()
        .fill(&mut ws, &volume, router.final_density(), &mut out);

    let count = ws.take_count();
    let total = count.evaluated + count.skipped;
    let share = count.skipped as f64 / total as f64;
    println!(
        "final_density over 16x384x16: {} of {total} node evaluations skipped ({:.1}%)",
        count.skipped,
        share * 100.0
    );
    assert!(
        share > 0.15,
        "the schedule skipped only {:.1}% of {total} evaluations",
        share * 100.0
    );
}
