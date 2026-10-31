use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_fixtures::{FixtureFile, fixture};
use mcrs_minecraft_noise::proto::NoiseParam;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::compile::build_router;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::{
    FINAL_DENSITY, NoiseGeneratorSettings, NoiseRouter, ROOT_NAMES, RouterBlocks,
};
use mcrs_minecraft_worldgen_testing as corpus;
use std::cmp::Ordering;
use std::collections::BTreeMap;

const MAGIC: &[u8; 8] = b"MCDFORCL";
/// The strict profile is the oracle and must reproduce the dumps bit for bit.
/// The fast profile reassociates and fuses roundings by design, so it is held to
/// a divergence budget instead — the divergence is measured, not waived. The
/// observed worst is 3.3e-6 on `final_density`; a break in the function itself
/// would be orders of magnitude wider.
#[cfg(not(feature = "fast"))]
const DIVERGENCE_BUDGET: Option<f32> = None;
#[cfg(feature = "fast")]
const DIVERGENCE_BUDGET: Option<f32> = Some(1.0e-5);

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

fn read_dump(name: &str, data: &[u8]) -> Dump {
    let mut r = Reader { data, pos: 0 };
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
    assert_eq!(r.pos, data.len(), "trailing bytes in {name}");
    Dump { seed, volumes }
}

fn lattice_dumps() -> Vec<FixtureFile> {
    fixture("density")
        .files()
        .into_iter()
        .filter(|file| !file.name().contains("dense"))
        .collect()
}

fn dense_dump() -> Dump {
    let file = fixture("density").file("overworld_s42_c0_0_dense.bin");
    read_dump(file.name(), &file.dump(MAGIC))
}

pub(crate) fn overworld_router(seed: u64) -> NoiseRouter {
    let settings: NoiseGeneratorSettings =
        corpus::read("noise_settings", &rl!("minecraft:overworld").to_arc());
    let registry: BTreeMap<ResourceLocation, DensityFunctionHolder> =
        corpus::registry("density_function");
    let noises: BTreeMap<ResourceLocation, NoiseParam> = corpus::registry("noise");
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
    )
    .expect("overworld router")
}

fn fill(router: &NoiseRouter, root: usize, v: &DumpVolume) -> Vec<f32> {
    let volume = SampleGrid::new(
        IVec3::from_array(v.size),
        IVec3::from_array(v.min),
        IVec3::from_array(v.step),
    );
    let mut out = vec![f32::NAN; volume.len()];
    let mut ws = Workspace::new();
    router.program.fill(&mut ws, &volume, root, &mut out);
    out
}

#[derive(Default)]
struct Diff {
    checked: usize,
    mismatched: usize,
    worst: f32,
    example: Option<(usize, f32, f32)>,
}

fn assert_supported_roots_match_the_vanilla_oracle(dumps: &[FixtureFile]) {
    let mut report: BTreeMap<String, Diff> = BTreeMap::new();
    let mut seen_names: Vec<String> = Vec::new();

    for file in dumps {
        let dump = read_dump(file.name(), &file.dump(MAGIC));
        let router = overworld_router(dump.seed as u64);

        for v in &dump.volumes {
            if !seen_names.contains(&v.name) {
                seen_names.push(v.name.clone());
            }
            let Some(root) = ROOT_NAMES.iter().position(|n| *n == v.name) else {
                continue;
            };
            let ours = fill(&router, root, v);
            assert_eq!(ours.len(), v.values.len(), "{} size", v.name);
            let slot = report.entry(v.name.clone()).or_default();
            for (i, (&got, &want)) in ours.iter().zip(&v.values).enumerate() {
                slot.checked += 1;
                if got.to_bits() != want.to_bits() {
                    let d = (got - want).abs();
                    if d.partial_cmp(&slot.worst).is_none_or(Ordering::is_gt) {
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

#[test]
fn supported_roots_match_the_vanilla_oracle_at_the_origin() {
    assert_supported_roots_match_the_vanilla_oracle(&[
        fixture("density").file("overworld_s42_c0_0.bin")
    ]);
}

mod exhaustive {
    use super::*;

    /// Every root the compiler supports, over every lattice dump.
    #[test]
    fn supported_roots_match_the_vanilla_oracle() {
        assert_supported_roots_match_the_vanilla_oracle(&lattice_dumps());
    }

    /// The dense dump is the only fixture that reaches the off-lattice interpolation
    /// path: on the lattice dumps the interpolated values are surface levels, exact
    /// multiples of eight, so no rounding happens and the interpolation order cannot
    /// be distinguished.
    #[test]
    fn final_density_matches_the_dense_oracle() {
        let dump = dense_dump();
        let router = overworld_router(dump.seed as u64);

        let v = dump
            .volumes
            .iter()
            .find(|v| v.name == "final_density")
            .expect("dense dump holds final_density");

        let ours = fill(&router, FINAL_DENSITY, v);
        let mut mismatched = 0usize;
        let mut worst = 0.0f32;
        let mut example = None;
        for (i, (&got, &want)) in ours.iter().zip(&v.values).enumerate() {
            if got.to_bits() != want.to_bits() {
                mismatched += 1;
                let d = (got - want).abs();
                if d.partial_cmp(&worst).is_none_or(Ordering::is_gt) {
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
}
