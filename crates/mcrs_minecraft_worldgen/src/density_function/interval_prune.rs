use super::*;
use crate::density_function::branch_schedule::reachable_backwards;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

const REL_EPS: f32 = 1e-4;
const ABS_EPS: f32 = 1e-6;
const DECIDE_EPS: f32 = 1e-3;
const ROWS_PER_BLOCK: usize = 4;

#[derive(Clone, Copy, Debug)]
struct Iv {
    lo: f32,
    hi: f32,
}

impl Iv {
    fn new(lo: f32, hi: f32) -> Self {
        Iv { lo, hi }
    }
    fn point(v: f32) -> Self {
        Iv { lo: v, hi: v }
    }
    fn widen(self) -> Self {
        Iv {
            lo: self.lo - (self.lo.abs() * REL_EPS + ABS_EPS),
            hi: self.hi + (self.hi.abs() * REL_EPS + ABS_EPS),
        }
    }
    fn hull(self, other: Iv) -> Self {
        Iv {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Class {
    Air,
    Stone,
    Undetermined,
}

#[derive(Default, Clone, Copy)]
struct Stats {
    rc_sites: u64,
    rc_resolved: u64,
    octaves_skipped: u64,
    octaves_total: u64,
    static_fallbacks: u64,
}

fn is_y_dependent_kind(c: &DensityFunctionComponent) -> bool {
    match c {
        DensityFunctionComponent::Independent(f) => matches!(
            f,
            IndependentDensityFunction::OldBlendedNoise(_)
                | IndependentDensityFunction::Noise(_)
                | IndependentDensityFunction::ClampedYGradient(_)
                | IndependentDensityFunction::Gradient(_)
                | IndependentDensityFunction::EndOuterIslands(_)
        ),
        DensityFunctionComponent::Dependent(f) => matches!(
            f,
            DependentDensityFunction::ShiftedNoise(_)
                | DependentDensityFunction::Slide(_)
                | DependentDensityFunction::Slice(_)
                | DependentDensityFunction::FindTopSurface(_)
        ),
        DensityFunctionComponent::Interpolated(_) => false,
    }
}

fn node_octaves(c: &DensityFunctionComponent) -> u64 {
    match c {
        DensityFunctionComponent::Independent(f) => match f {
            IndependentDensityFunction::OldBlendedNoise(_) => 40,
            IndependentDensityFunction::Noise(x) => x.sampler.octave_count() as u64,
            IndependentDensityFunction::ShiftA(x) => x.sampler.octave_count() as u64,
            IndependentDensityFunction::ShiftB(x) => x.sampler.octave_count() as u64,
            IndependentDensityFunction::Shift(x) => x.sampler.octave_count() as u64,
            _ => 0,
        },
        DensityFunctionComponent::Dependent(DependentDensityFunction::ShiftedNoise(x)) => {
            x.sampler.octave_count() as u64
        }
        _ => 0,
    }
}

fn grad_iv(g: &ClampedYGradient, y_lo: f32, y_hi: f32) -> Iv {
    let a = g.sample(IVec3::new(0, y_lo as i32, 0));
    let b = g.sample(IVec3::new(0, y_hi as i32, 0));
    Iv::new(a.min(b), a.max(b))
}

/// Per-RangeChoice cone data: octaves exclusive to each arm.
struct RcSite {
    index: usize,
    when_in_octaves: u64,
    when_out_octaves: u64,
}

fn rc_sites(router: &NoiseRouter) -> Vec<RcSite> {
    let cb = router.column_boundary;
    let fd = router.final_density_index;
    let stack = &router.stack;
    let mut out = Vec::new();
    for rc_idx in cb..=fd {
        let rc = match &stack[rc_idx] {
            DensityFunctionComponent::Dependent(DependentDensityFunction::RangeChoice(rc)) => rc,
            _ => continue,
        };
        let extent = fd + 1;
        let wi = reachable_backwards(rc.when_in_index, stack, extent);
        let wo = reachable_backwards(rc.when_out_index, stack, extent);
        let without = {
            let mut visited = vec![false; extent];
            visited[fd] = true;
            for i in (cb..=fd).rev() {
                if !visited[i] {
                    continue;
                }
                if i == rc_idx {
                    visited[rc.input_index] = true;
                } else {
                    stack[i].visit_input_indices(&mut |dep| {
                        if dep <= fd {
                            visited[dep] = true;
                        }
                    });
                }
            }
            visited
        };
        let octaves = |keep: &Vec<bool>, drop: &Vec<bool>| -> u64 {
            (cb..rc_idx)
                .filter(|&e| keep[e] && !drop[e] && !without[e])
                .map(|e| node_octaves(&stack[e]))
                .sum()
        };
        out.push(RcSite {
            index: rc_idx,
            when_in_octaves: octaves(&wi, &wo),
            when_out_octaves: octaves(&wo, &wi),
        });
    }
    out
}

/// Every column-invariant value at `(x, z)`, in stack order.
fn zone_a_at(router: &NoiseRouter, x: i32, z: i32, scratch: &mut FillScratch) -> Vec<f32> {
    let entries: Vec<usize> = (0..router.column_boundary).collect();
    let mut out = vec![0.0f32; entries.len()];
    router.sample_volume_roots(
        &entries,
        &Volume::point(IVec3::new(x, 0, z)),
        &mut out,
        scratch,
    );
    out
}

struct Walker<'a> {
    router: &'a NoiseRouter,
    iv: Vec<Iv>,
    pt: Vec<f32>,
    reg: Vec<f32>,
    exact: Vec<bool>,
    rc_by_index: Vec<Option<usize>>,
    sites: Vec<RcSite>,
}

impl<'a> Walker<'a> {
    fn new(router: &'a NoiseRouter) -> Self {
        let n = router.stack.len();
        let sites = rc_sites(router);
        let mut rc_by_index = vec![None; n];
        for (i, s) in sites.iter().enumerate() {
            rc_by_index[s.index] = Some(i);
        }
        Walker {
            router,
            iv: vec![Iv::point(0.0); n],
            pt: vec![0.0; n],
            reg: vec![0.0; n],
            exact: vec![false; n],
            rc_by_index,
            sites,
        }
    }

    fn seed(&mut self, zone_a: &[f32]) {
        for i in 0..self.router.column_boundary {
            self.iv[i] = Iv::point(zone_a[i]);
            self.pt[i] = zone_a[i];
            self.exact[i] = true;
        }
    }

    fn walk(&mut self, x: i32, z: i32, y_lo: i32, y_hi: i32, stats: &mut Stats) -> Iv {
        let router = self.router;
        let stack = &router.stack;
        let cb = router.column_boundary;
        let fd = router.final_density_index;
        let y_degenerate = y_lo == y_hi;
        let pos = IVec3::new(x, y_lo, z);

        for i in cb..=fd {
            let comp = &stack[i];

            let mut inputs_exact = true;
            comp.visit_input_indices(&mut |dep| {
                if !self.exact[dep] {
                    inputs_exact = false;
                }
            });
            if inputs_exact && (y_degenerate || !is_y_dependent_kind(comp)) {
                volume::Arena::new(stack).fill_node(
                    i,
                    &Volume::point(pos),
                    &[pos],
                    &mut self.pt,
                    &mut self.reg,
                );
                let v = self.pt[i];
                self.exact[i] = true;
                self.iv[i] = Iv::point(v).widen();
                continue;
            }
            self.exact[i] = false;

            let statik = || Iv::new(comp.min_value(), comp.max_value());
            let raw = match comp {
                DensityFunctionComponent::Independent(f) => match f {
                    IndependentDensityFunction::Constant(v) => Iv::point(*v),
                    IndependentDensityFunction::ClampedYGradient(g) => {
                        grad_iv(g, y_lo as f32, y_hi as f32)
                    }
                    IndependentDensityFunction::Gradient(g)
                        if g.axis == Axis::Y && g.tiling == TilingMode::ClampToEdge =>
                    {
                        let a = g.sample(IVec3::new(x, y_lo, z));
                        let b = g.sample(IVec3::new(x, y_hi, z));
                        Iv::new(a.min(b), a.max(b))
                    }
                    IndependentDensityFunction::Gradient(g) if g.axis != Axis::Y => {
                        Iv::point(g.sample(pos))
                    }
                    _ => {
                        stats.static_fallbacks += 1;
                        statik()
                    }
                },
                DensityFunctionComponent::Dependent(f) => match f {
                    DependentDensityFunction::Linear(x2) => {
                        let a = self.iv[x2.input_index];
                        match x2.operation {
                            LinearOperation::Add => Iv::new(a.lo + x2.argument, a.hi + x2.argument),
                            LinearOperation::Multiply => {
                                let (lo, hi) = mul_range(a.lo, a.hi, x2.argument, x2.argument);
                                Iv::new(lo, hi)
                            }
                        }
                    }
                    DependentDensityFunction::Affine(x2) => {
                        let a = self.iv[x2.input_index];
                        let (lo, hi) = Affine::compute_range(a.lo, a.hi, x2.scale, x2.offset);
                        Iv::new(lo, hi)
                    }
                    DependentDensityFunction::PiecewiseAffine(x2) => {
                        let a = self.iv[x2.input_index];
                        let (lo, hi) = PiecewiseAffine::compute_range(
                            a.lo,
                            a.hi,
                            x2.neg_scale,
                            x2.pos_scale,
                            x2.offset,
                        );
                        Iv::new(lo, hi)
                    }
                    DependentDensityFunction::Slide(x2) => {
                        let a = self.iv[x2.input_index];
                        let g1 = grad_iv(&x2.grad1, y_lo as f32, y_hi as f32);
                        let g2 = grad_iv(&x2.grad2, y_lo as f32, y_hi as f32);
                        let inner = Iv::new(a.lo + x2.offset_a, a.hi + x2.offset_a);
                        let (p1lo, p1hi) = mul_range(g1.lo, g1.hi, inner.lo, inner.hi);
                        let (p2lo, p2hi) =
                            mul_range(p1lo + x2.offset_b, p1hi + x2.offset_b, g2.lo, g2.hi);
                        Iv::new(p2lo + x2.offset_c, p2hi + x2.offset_c)
                    }
                    DependentDensityFunction::Unary(x2) => {
                        let a = self.iv[x2.input_index];
                        let (lo, hi) = unary_range(x2.operation, a.lo, a.hi);
                        Iv::new(lo, hi)
                    }
                    DependentDensityFunction::Binary(x2) => {
                        let a = self.iv[x2.input1_index];
                        let b = self.iv[x2.input2_index];
                        let (lo, hi) = binary_range(x2.operation, (a.lo, a.hi), (b.lo, b.hi));
                        Iv::new(lo, hi)
                    }
                    DependentDensityFunction::Clamp(x2) => {
                        let a = self.iv[x2.input_index];
                        Iv::new(
                            a.lo.clamp(x2.min_value, x2.max_value),
                            a.hi.clamp(x2.min_value, x2.max_value),
                        )
                    }
                    DependentDensityFunction::RangeChoice(x2) => {
                        let a = self.iv[x2.input_index];
                        let wi = self.iv[x2.when_in_index];
                        let wo = self.iv[x2.when_out_index];
                        let site = self.rc_by_index[i].map(|s| &self.sites[s]);
                        stats.rc_sites += 1;
                        if let Some(s) = site {
                            stats.octaves_total += s.when_in_octaves + s.when_out_octaves;
                        }
                        if a.lo >= x2.min_inclusion_value && a.hi < x2.max_exclusion_value {
                            stats.rc_resolved += 1;
                            if let Some(s) = site {
                                stats.octaves_skipped += s.when_out_octaves;
                            }
                            wi
                        } else if a.hi < x2.min_inclusion_value || a.lo >= x2.max_exclusion_value {
                            stats.rc_resolved += 1;
                            if let Some(s) = site {
                                stats.octaves_skipped += s.when_in_octaves;
                            }
                            wo
                        } else {
                            wi.hull(wo)
                        }
                    }
                    DependentDensityFunction::Lerp(x2) => {
                        let al = self.iv[x2.alpha_index];
                        let first = self.iv[x2.first_index];
                        let second = self.iv[x2.second_index];
                        let d = Iv::new(second.lo - first.hi, second.hi - first.lo);
                        let (m_lo, m_hi) = mul_range(al.lo, al.hi, d.lo, d.hi);
                        Iv::new(first.lo + m_lo, first.hi + m_hi)
                    }
                    _ => {
                        stats.static_fallbacks += 1;
                        statik()
                    }
                },
                DensityFunctionComponent::Interpolated(x2) => self.iv[x2.input_index],
            };
            self.iv[i] = raw.widen();
        }
        self.iv[fd]
    }
}

fn classify(iv: Iv) -> Class {
    if iv.hi <= -DECIDE_EPS {
        Class::Air
    } else if iv.lo > DECIDE_EPS {
        Class::Stone
    } else {
        Class::Undetermined
    }
}

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
            let ident = ResourceLocation::parse(&format!("minecraft:{}", name)).unwrap();
            out.push((ident, std::fs::read(&path).unwrap()));
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
        VoxelId(1),
        VoxelId(86),
    )
}

struct ColumnResult {
    air: usize,
    stone: usize,
    undetermined: usize,
    oracle_air: usize,
    oracle_stone: usize,
    bisect_walks: usize,
}

fn run_seed(seed: u64, radius: i32) -> (Vec<ColumnResult>, Stats, f64, f64, u64) {
    let router = overworld_router(seed);
    let v = router
        .cell_size()
        .expect("the overworld interpolates on one lattice")
        .y;
    let rows = router.noise_height() as usize / v as usize + 1;
    let min_y = router.noise_min_y();

    let mut walker = Walker::new(&router);
    let mut stats = Stats::default();
    let mut columns = Vec::new();
    let mut walk_nanos = 0u128;
    let mut populate_nanos = 0u128;
    let mut pruned_nanos = 0u128;
    let mut kept_nanos = 0u128;
    let t_all = Instant::now();
    let mut eval_nanos = 0u128;
    let mut violations = 0u64;
    let mut values = vec![0.0f32; rows];
    let mut scratch = FillScratch::new();

    for cx in -radius..=radius {
        for cz in -radius..=radius {
            let bx = cx * 16;
            let bz = cz * 16;
            let t_pop = Instant::now();
            let zone_a = zone_a_at(&router, bx, bz, &mut scratch);
            populate_nanos += t_pop.elapsed().as_nanos();

            let mut result = ColumnResult {
                air: 0,
                stone: 0,
                undetermined: 0,
                oracle_air: 0,
                oracle_stone: 0,
                bisect_walks: 0,
            };

            let t = Instant::now();
            let mut classes = Vec::new();
            let mut r0 = 0;
            while r0 < rows {
                let r1 = if rows - r0 <= ROWS_PER_BLOCK + 1 {
                    rows
                } else {
                    r0 + ROWS_PER_BLOCK
                };
                walker.seed(&zone_a);
                let iv = walker.walk(
                    bx,
                    bz,
                    min_y + r0 as i32 * v,
                    min_y + (r1 - 1) as i32 * v,
                    &mut stats,
                );
                classes.push((r0, r1, classify(iv)));
                r0 = r1;
            }
            walk_nanos += t.elapsed().as_nanos();

            let t = Instant::now();
            for &(r0, r1, class) in classes.iter() {
                if class == Class::Undetermined {
                    continue;
                }
                for r in r0..r1 {
                    values[r] = router.sample_value(
                        router.final_density_index,
                        IVec3::new(bx, min_y + r as i32 * v, bz),
                        &mut scratch,
                    );
                }
            }
            let pruned_elapsed = t.elapsed().as_nanos();
            pruned_nanos += pruned_elapsed;
            black_box(&values);

            let t = Instant::now();
            for &(r0, r1, class) in classes.iter() {
                if class != Class::Undetermined {
                    continue;
                }
                for r in r0..r1 {
                    values[r] = router.sample_value(
                        router.final_density_index,
                        IVec3::new(bx, min_y + r as i32 * v, bz),
                        &mut scratch,
                    );
                }
            }
            kept_nanos += t.elapsed().as_nanos();
            eval_nanos += pruned_elapsed + t.elapsed().as_nanos();
            black_box(&values);

            for &(r0, r1, _) in classes.iter() {
                if values[r0..r1].iter().all(|&v| v <= 0.0) {
                    result.oracle_air += r1 - r0;
                } else if values[r0..r1].iter().all(|&v| v > 0.0) {
                    result.oracle_stone += r1 - r0;
                }
            }

            for (r0, r1, class) in classes {
                let n = r1 - r0;
                match class {
                    Class::Air => {
                        result.air += n;
                        for r in r0..r1 {
                            if values[r] > 0.0 {
                                violations += 1;
                                eprintln!(
                                    "VIOLATION air block at chunk ({cx},{cz}) row {r} value {}",
                                    values[r]
                                );
                            }
                        }
                    }
                    Class::Stone => {
                        result.stone += n;
                        for r in r0..r1 {
                            if values[r] <= 0.0 {
                                violations += 1;
                                eprintln!(
                                    "VIOLATION stone block at chunk ({cx},{cz}) row {r} value {}",
                                    values[r]
                                );
                            }
                        }
                    }
                    Class::Undetermined => result.undetermined += n,
                }
            }

            let mut scratch_stats = Stats::default();
            result.bisect_walks = bisect(
                &mut walker,
                &zone_a,
                bx,
                bz,
                min_y,
                v,
                0,
                rows,
                &mut scratch_stats,
            );
            columns.push(result);
        }
    }

    println!(
        "  [rows] evaluating the pruned rows costs {:.1} ms, the kept rows {:.1} ms",
        pruned_nanos as f64 / 1e6,
        kept_nanos as f64 / 1e6,
    );
    println!(
        "  [phases] total {:.1} ms = populate {:.1} + eval {:.1} + walk {:.1} + bisect/other {:.1}",
        t_all.elapsed().as_nanos() as f64 / 1e6,
        populate_nanos as f64 / 1e6,
        eval_nanos as f64 / 1e6,
        walk_nanos as f64 / 1e6,
        (t_all.elapsed().as_nanos() - populate_nanos - eval_nanos - walk_nanos) as f64 / 1e6,
    );
    (
        columns,
        stats,
        walk_nanos as f64 / 1e6,
        eval_nanos as f64 / 1e6,
        violations,
    )
}

#[allow(clippy::too_many_arguments)]
fn bisect(
    walker: &mut Walker,
    zone_a: &[f32],
    bx: i32,
    bz: i32,
    min_y: i32,
    v: i32,
    r0: usize,
    r1: usize,
    stats: &mut Stats,
) -> usize {
    walker.seed(zone_a);
    let iv = walker.walk(
        bx,
        bz,
        min_y + r0 as i32 * v,
        min_y + (r1 - 1) as i32 * v,
        stats,
    );
    if classify(iv) != Class::Undetermined || (r1 - r0) <= ROWS_PER_BLOCK {
        return 1;
    }
    let mid = r0 + (r1 - r0) / 2;
    1 + bisect(walker, zone_a, bx, bz, min_y, v, r0, mid, stats)
        + bisect(walker, zone_a, bx, bz, min_y, v, mid, r1, stats)
}

#[test]
#[ignore]
fn interval_prune_yield() {
    let radius = 10;
    for seed in [0u64, 1234567890u64] {
        let (columns, stats, walk_ms, eval_ms, violations) = run_seed(seed, radius);
        let n_cols = columns.len();
        let rows_total: usize = columns
            .iter()
            .map(|c| c.air + c.stone + c.undetermined)
            .sum();
        let air: usize = columns.iter().map(|c| c.air).sum();
        let stone: usize = columns.iter().map(|c| c.stone).sum();
        let undet: usize = columns.iter().map(|c| c.undetermined).sum();

        let mut per_col: Vec<f64> = columns
            .iter()
            .map(|c| (c.air + c.stone) as f64 / (c.air + c.stone + c.undetermined) as f64 * 100.0)
            .collect();
        per_col.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f64| per_col[((per_col.len() - 1) as f64 * p) as usize];
        let mean_col = per_col.iter().sum::<f64>() / per_col.len() as f64;
        let zero_cols = per_col.iter().filter(|&&v| v == 0.0).count();

        let bisect_mean: f64 =
            columns.iter().map(|c| c.bisect_walks as f64).sum::<f64>() / n_cols as f64;

        println!("=== seed {seed}, {n_cols} columns, {rows_total} lattice rows ===");
        let oracle_air: usize = columns.iter().map(|c| c.oracle_air).sum();
        let oracle_stone: usize = columns.iter().map(|c| c.oracle_stone).sum();
        println!(
            "  oracle ceiling at this block size: air {:.2}%  stone {:.2}%  pruned {:.2}%",
            oracle_air as f64 / rows_total as f64 * 100.0,
            oracle_stone as f64 / rows_total as f64 * 100.0,
            (oracle_air + oracle_stone) as f64 / rows_total as f64 * 100.0
        );
        println!(
            "  rows: air {:.2}%  stone {:.2}%  pruned {:.2}%  undetermined {:.2}%",
            air as f64 / rows_total as f64 * 100.0,
            stone as f64 / rows_total as f64 * 100.0,
            (air + stone) as f64 / rows_total as f64 * 100.0,
            undet as f64 / rows_total as f64 * 100.0
        );
        println!(
            "  per-column pruned %: mean {:.2}  p5 {:.2}  p25 {:.2}  median {:.2}  p75 {:.2}  p95 {:.2}  min {:.2}  max {:.2}  columns with 0% {}",
            mean_col,
            pct(0.05),
            pct(0.25),
            pct(0.50),
            pct(0.75),
            pct(0.95),
            per_col[0],
            per_col[per_col.len() - 1],
            zero_cols
        );
        println!(
            "  range_choice sites visited {}  resolved to one arm {} ({:.2}%)",
            stats.rc_sites,
            stats.rc_resolved,
            stats.rc_resolved as f64 / stats.rc_sites as f64 * 100.0
        );
        println!(
            "  octaves in rc exclusive cones {}  skipped by resolution {} ({:.2}%)",
            stats.octaves_total,
            stats.octaves_skipped,
            stats.octaves_skipped as f64 / stats.octaves_total.max(1) as f64 * 100.0
        );
        println!("  static fallbacks during walk: {}", stats.static_fallbacks);
        println!(
            "  walk cost {:.1} ms vs full column eval {:.1} ms = {:.2}% of column time",
            walk_ms,
            eval_ms,
            walk_ms / eval_ms * 100.0
        );
        println!("  bisection walks per column: mean {:.2}", bisect_mean);
        println!("  CORRECTNESS violations: {violations}");
        assert_eq!(violations, 0, "interval algebra unsound");
    }
}

fn kind_name(c: &DensityFunctionComponent) -> &'static str {
    match c {
        DensityFunctionComponent::Independent(f) => match f {
            IndependentDensityFunction::Constant(_) => "Constant",
            IndependentDensityFunction::OldBlendedNoise(_) => "OldBlendedNoise",
            IndependentDensityFunction::Noise(_) => "Noise",
            IndependentDensityFunction::ShiftA(_) => "ShiftA",
            IndependentDensityFunction::ShiftB(_) => "ShiftB",
            IndependentDensityFunction::Shift(_) => "Shift",
            IndependentDensityFunction::ClampedYGradient(_) => "ClampedYGradient",
            IndependentDensityFunction::Gradient(_) => "Gradient",
            IndependentDensityFunction::DistanceToPoint(_) => "DistanceToPoint",
            IndependentDensityFunction::EndOuterIslands(_) => "EndOuterIslands",
        },
        DensityFunctionComponent::Dependent(f) => match f {
            DependentDensityFunction::Linear(_) => "Linear",
            DependentDensityFunction::Affine(_) => "Affine",
            DependentDensityFunction::PiecewiseAffine(_) => "PiecewiseAffine",
            DependentDensityFunction::Slide(_) => "Slide",
            DependentDensityFunction::Unary(_) => "Unary",
            DependentDensityFunction::Binary(_) => "Binary",
            DependentDensityFunction::ShiftedNoise(_) => "ShiftedNoise",
            DependentDensityFunction::Clamp(_) => "Clamp",
            DependentDensityFunction::RangeChoice(_) => "RangeChoice",
            DependentDensityFunction::Spline(_) => "Spline",
            DependentDensityFunction::FindTopSurface(_) => "FindTopSurface",
            DependentDensityFunction::Lerp(_) => "Lerp",
            DependentDensityFunction::Slice(_) => "Slice",
        },
        DensityFunctionComponent::Interpolated(_) => "Interpolated",
    }
}

#[test]
#[ignore]
fn interval_prune_debug() {
    let router = overworld_router(0);
    let cb = router.column_boundary;
    let fd = router.final_density_index;
    println!("zone A {cb} entries, zone B {} entries", fd + 1 - cb);
    let mut hist: BTreeMap<&str, usize> = BTreeMap::new();
    for i in cb..=fd {
        *hist.entry(kind_name(&router.stack[i])).or_default() += 1;
    }
    println!("zone B kinds: {hist:?}");
    let mut scratch = FillScratch::new();
    let zone_a = zone_a_at(&router, 0, 0, &mut scratch);
    let mut walker = Walker::new(&router);
    let mut stats = Stats::default();
    for (y_lo, y_hi) in [(0, 24), (-56, -32), (120, 144)] {
        walker.seed(&zone_a);
        let iv = walker.walk(0, 0, y_lo, y_hi, &mut stats);
        println!("--- y {y_lo}..{y_hi}: final [{:.4}, {:.4}]", iv.lo, iv.hi);
        for i in cb..=fd {
            let w = walker.iv[i].hi - walker.iv[i].lo;
            if w > 0.05 {
                println!(
                    "  [{i}] {:<16} {:<44} [{:>9.4}, {:>9.4}] static [{:>9.4}, {:>9.4}]",
                    kind_name(&router.stack[i]),
                    router.node_labels[i],
                    walker.iv[i].lo,
                    walker.iv[i].hi,
                    router.stack[i].min_value(),
                    router.stack[i].max_value(),
                );
            }
        }
        let mut real = vec![0.0f32; 4];
        for (k, r) in real.iter_mut().enumerate() {
            *r = router.sample_value(fd, IVec3::new(0, y_lo + k as i32 * 8, 0), &mut scratch);
        }
        println!("  actual values: {real:?}");
    }
}

/// Walk the branch schedule at real corner positions and report how much of the
/// noise work the guards actually remove.
#[test]
#[ignore]
fn branch_skip_octave_census() {
    use crate::density_function::branch_schedule::Step;

    let router = overworld_router(845);
    let cb = router.column_boundary;
    let fd = router.final_density_index;
    let sched = &router.zone_b_schedule;
    let per_pos_total: u64 = (cb..=fd).map(|i| node_octaves(&router.stack[i])).sum();

    let guards = sched
        .steps
        .iter()
        .filter(|s| matches!(s, Step::Guard { .. }))
        .count();
    let guarded_nodes = sched.guarded.iter().filter(|&&g| g).count();
    println!(
        "zone B {} nodes, {per_pos_total} octaves/position, {guards} guards over {guarded_nodes} guarded nodes",
        fd + 1 - cb
    );

    let mut evaluated = 0u64;
    let mut total = 0u64;
    let mut positions = 0u64;
    let mut sites = 0u64;
    let mut taken = 0u64;

    let mut scratch = FillScratch::new();
    let n = router.stack.len();
    let mut pt = vec![0.0f32; n];
    let mut reg = vec![0.0f32; n];
    let arena = volume::Arena::new(&router.stack);
    for chunk in 0..4i32 {
        let (bx, bz) = (chunk * 16, chunk * 48);
        for gx in 0..5i32 {
            for gz in 0..5i32 {
                let zone_a = zone_a_at(&router, bx + gx * 4, bz + gz * 4, &mut scratch);
                for row in 0..49i32 {
                    pt[..cb].copy_from_slice(&zone_a);
                    let pos = IVec3::new(bx + gx * 4, -64 + row * 8, bz + gz * 4);
                    positions += 1;
                    total += per_pos_total;
                    let mut s = 0usize;
                    while s < sched.steps.len() {
                        match sched.steps[s] {
                            Step::Eval { start, end } => {
                                for &i in &sched.order[start as usize..end as usize] {
                                    evaluated += node_octaves(&router.stack[i]);
                                    arena.fill_node(
                                        i,
                                        &Volume::point(pos),
                                        &[pos],
                                        &mut pt,
                                        &mut reg,
                                    );
                                }
                                s += 1;
                            }
                            Step::Guard {
                                input,
                                min_inclusive,
                                max_exclusive,
                                want_in,
                                unguard,
                            } => {
                                sites += 1;
                                let v = pt[input as usize];
                                let hit = (v >= min_inclusive && v < max_exclusive) == want_in;
                                taken += hit as u64;
                                s = if hit { s + 1 } else { unguard as usize };
                            }
                            Step::Unguard => s += 1,
                        }
                    }
                }
            }
        }
    }

    let skipped = total - evaluated;
    println!(
        "{positions} corner positions: {evaluated}/{total} octaves evaluated, {skipped} skipped ({:.1}%)",
        skipped as f64 / total as f64 * 100.0
    );
    println!(
        "guard visits {sites}, cone entered {taken} ({:.1}%), cone jumped {} ({:.1}%)",
        taken as f64 / sites as f64 * 100.0,
        sites - taken,
        (sites - taken) as f64 / sites as f64 * 100.0
    );
    assert!(skipped * 5 > total, "branch skipping lost its effect");
}
