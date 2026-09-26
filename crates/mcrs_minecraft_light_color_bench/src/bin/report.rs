use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::region::{Palette, Region, section_output};
use mcrs_minecraft_light_color_bench::candidates::{CANDIDATES, Outcome, Stages, mismatch};
use mcrs_minecraft_light_color_bench::fixture::{Scene, oracle, relaxed_block_light, scenes};
use mcrs_minecraft_light_color_bench::shade::{final_rgb, hue, hue_difference};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(bytes: usize) {
    let now = CURRENT.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

fn shrink(bytes: usize) {
    CURRENT.fetch_sub(bytes, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        shrink(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(ptr, layout, new_size) };
        if !moved.is_null() {
            if new_size > layout.size() {
                grow(new_size - layout.size());
            } else {
                shrink(layout.size() - new_size);
            }
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const REPETITIONS: usize = 20;
const RENDER_DISTANCE_COLUMNS: f64 = 65.0 * 65.0;
const SLICE: usize = 48;
const SCALE: usize = 8;
const OUTPUT: usize = 18;
const UNLIT: [u8; 4] = [0, 0, 0, 255];

#[derive(Default)]
struct Measured {
    lit: usize,
    lanes: bool,
    mismatch: Option<String>,
    samples: Vec<Stages>,
    peak: usize,
    delta_max: u8,
    delta_sum: u64,
    delta_count: u64,
    hue_max: f32,
    hue_sum: f64,
    hue_count: u64,
    texels: HashMap<SectionPos, Box<[[u8; 4]]>>,
}

fn main() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/light_color_bench");
    std::fs::create_dir_all(&out).expect("the report directory is writable");
    let out = out.canonicalize().expect("the report directory resolves");

    let mut table = String::from(
        "| scene | candidate | lit sections | exact | snapshot | costs | propagation | resolve \
         | median | p99 | peak memory | max Δ | mean Δ | max hue Δ | mean hue Δ | join estimate \
         | redundancy |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let mut notes = String::new();

    for scene in scenes() {
        let measured = measure(&scene, &out);
        let sections = (scene.bounds.max_section_y - scene.bounds.min_section_y + 1) as f64;
        for (candidate, m) in CANDIDATES.iter().zip(&measured) {
            let stage = |pick: fn(&Stages) -> Duration| {
                let mut values: Vec<Duration> = m.samples.iter().map(pick).collect();
                ms(percentile(&mut values, 0.5))
            };
            let mut totals: Vec<Duration> = m.samples.iter().map(Stages::total).collect();
            let median = percentile(&mut totals, 0.5);
            let p99 = percentile(&mut totals, 0.99);
            let join =
                median.as_secs_f64() * m.lit as f64 / 27.0 * RENDER_DISTANCE_COLUMNS * sections;
            let exact = match (&m.mismatch, m.lanes) {
                (Some(first), _) => format!("no ({first})"),
                (None, true) => "yes".to_owned(),
                (None, false) => "n/a".to_owned(),
            };
            writeln!(
                table,
                "| {} | {} | {} | {exact} | {} | {} | {} | {} | {} | {} | {:.2} MiB | {} | {:.3} \
                 | {:.1}° | {:.2}° | {join:.1} s | {:.2}× |",
                scene.name,
                candidate.name,
                m.lit,
                stage(|s| s.snapshot),
                stage(|s| s.costs),
                stage(|s| s.propagation),
                stage(|s| s.resolve),
                ms(median),
                ms(p99),
                m.peak as f64 / (1024.0 * 1024.0),
                m.delta_max,
                m.delta_sum as f64 / m.delta_count.max(1) as f64,
                m.hue_max,
                m.hue_sum / m.hue_count.max(1) as f64,
                candidate.redundancy,
            )
            .unwrap();
        }
        writeln!(notes, "- {}", saved_light_note(&scene)).unwrap();
    }

    let report = format!(
        "# Light colour propagation\n\n\
         Native release build, single process. Each lit inner section runs once to warm up and \
         to measure memory, then {REPETITIONS} times with the candidates interleaved. Stage \
         columns are medians; median and p99 are of the total per section. Deltas are the final \
         block-light RGB against the reference over cells whose server level is above 0. The \
         join estimate is the median per lit section times the lit share of the 27 inner \
         sections, times 65² columns, times the dimension's section count. `bfs` is the server's \
         relax, which spreads any round of 4096 or more cells over the rayon pool, so its \
         times on dense emitters use several threads; every other stage runs on one.\n\n\
         {table}\n\
         Server level against relax over every light type on the 27 inner sections:\n\n\
         {notes}"
    );
    print!("{table}\n{notes}");
    let path = out.join("report.md");
    std::fs::write(&path, report).expect("the report is writable");
    println!("report: {}", path.display());
}

fn measure(scene: &Scene, out: &Path) -> Vec<Measured> {
    let mut measured: Vec<Measured> = CANDIDATES.iter().map(|_| Measured::default()).collect();

    let lit: Vec<SectionPos> = scene.inner().filter(|&s| has_emitters(scene, s)).collect();
    for &section in &lit {
        let outcomes: Vec<Option<Outcome>> = CANDIDATES
            .iter()
            .zip(&mut measured)
            .map(|(candidate, m)| {
                let base = CURRENT.load(Ordering::Relaxed);
                PEAK.store(base, Ordering::Relaxed);
                let outcome = (candidate.run)(scene, section, &mut Stages::default());
                m.peak = m
                    .peak
                    .max(PEAK.load(Ordering::Relaxed).saturating_sub(base));
                outcome
            })
            .collect();

        let types: BTreeSet<LightType> = outcomes
            .iter()
            .flatten()
            .flat_map(|o| o.lanes.iter().flatten().map(|(t, _)| *t))
            .collect();
        let want: HashMap<LightType, Vec<u8>> = types
            .into_iter()
            .map(|t| (t, oracle(scene, section, t)))
            .collect();

        let reference = outcomes[0].as_ref().map(|o| &o.texels[..]);
        for (outcome, m) in outcomes.iter().zip(&mut measured) {
            m.lit += outcome.is_some() as usize;
            if let Some(lanes) = outcome.as_ref().and_then(|o| o.lanes.as_ref()) {
                m.lanes = true;
                if m.mismatch.is_none() {
                    m.mismatch = mismatch(section, lanes, |t| want[&t].clone());
                }
            }
            let texels = outcome.as_ref().map(|o| &o.texels[..]);
            compare(scene, section, reference, texels, m);
            if let Some(outcome) = outcome {
                m.texels.insert(section, outcome.texels.clone());
            }
        }
        drop(outcomes);

        for _ in 0..REPETITIONS {
            for (candidate, m) in CANDIDATES.iter().zip(&mut measured) {
                let mut stages = Stages::default();
                let outcome = (candidate.run)(scene, section, &mut stages);
                drop(outcome);
                m.samples.push(stages);
            }
        }
    }

    let layer = most_varied_layer(scene, &measured[0].texels);
    let reference = slice(scene, layer, &measured[0].texels);
    for (candidate, m) in CANDIDATES.iter().zip(&measured) {
        let image = slice(scene, layer, &m.texels);
        write_png(
            &out.join(format!("{}-{}.png", scene.name, candidate.name)),
            &image,
        );
        let diff: Vec<u8> = image
            .iter()
            .zip(&reference)
            .map(|(&a, &b)| a.abs_diff(b).saturating_mul(8))
            .collect();
        write_png(
            &out.join(format!("{}-{}-diff.png", scene.name, candidate.name)),
            &diff,
        );
    }
    measured
}

fn has_emitters(scene: &Scene, section: SectionPos) -> bool {
    let (min, size) = section_output(section);
    let region = Region::new(min, size, scene.bounds, &scene.registry, scene.cells());
    !Palette::of(&region, &scene.registry, &scene.colours)
        .types
        .is_empty()
}

fn output_index(section: SectionPos, pos: BlockPos) -> usize {
    let (min, _) = section_output(section);
    let d = pos - min.as_ivec3();
    (d.x + OUTPUT as i32 * (d.z + OUTPUT as i32 * d.y)) as usize
}

fn compare(
    scene: &Scene,
    section: SectionPos,
    reference: Option<&[[u8; 4]]>,
    candidate: Option<&[[u8; 4]]>,
    m: &mut Measured,
) {
    let base = section.0 * SectionPos::SIZE as i32;
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let pos = BlockPos::new(base.x + x, base.y + y, base.z + z);
                let level = scene.server_level(pos);
                if level == 0 {
                    continue;
                }
                let i = output_index(section, pos);
                let texel = |o: Option<&[[u8; 4]]>| o.map_or(UNLIT, |t| t[i]);
                let want = final_rgb(texel(reference), level);
                let got = final_rgb(texel(candidate), level);
                for (a, b) in got.iter().zip(&want) {
                    let d = a.abs_diff(*b);
                    m.delta_max = m.delta_max.max(d);
                    m.delta_sum += d as u64;
                    m.delta_count += 1;
                }
                if let (Some(a), Some(b)) = (hue(got), hue(want)) {
                    let d = hue_difference(a, b);
                    m.hue_max = m.hue_max.max(d);
                    m.hue_sum += d as f64;
                    m.hue_count += 1;
                }
            }
        }
    }
}

/// The inner layer whose reference slice holds the most distinct colours. A
/// lava ocean lights each layer above it almost evenly, so the busiest layer
/// by lit cells can be a flat image.
fn most_varied_layer(scene: &Scene, reference: &HashMap<SectionPos, Box<[[u8; 4]]>>) -> i32 {
    let min = scene.inner_min();
    (min.y..min.y + SLICE as i32)
        .max_by_key(|&y| {
            let colours: HashSet<[u8; 3]> = slice(scene, y, reference)
                .as_chunks::<{ 3 * SCALE }>()
                .0
                .iter()
                .map(|pixels| [pixels[0], pixels[1], pixels[2]])
                .collect();
            (colours.len(), std::cmp::Reverse(y))
        })
        .unwrap()
}

/// Final RGB of the inner 48×48 cells at `y`, scaled up for viewing.
fn slice(scene: &Scene, y: i32, texels: &HashMap<SectionPos, Box<[[u8; 4]]>>) -> Vec<u8> {
    let min = scene.inner_min();
    let side = SLICE * SCALE;
    let mut rgb = vec![0u8; side * side * 3];
    for z in 0..SLICE {
        for x in 0..SLICE {
            let pos = BlockPos::new(min.x + x as i32, y, min.z + z as i32);
            let section = SectionPos::from(pos);
            let texel = texels
                .get(&section)
                .map_or(UNLIT, |t| t[output_index(section, pos)]);
            let colour = final_rgb(texel, scene.server_level(pos));
            for dz in 0..SCALE {
                for dx in 0..SCALE {
                    let at = ((z * SCALE + dz) * side + x * SCALE + dx) * 3;
                    rgb[at..at + 3].copy_from_slice(&colour);
                }
            }
        }
    }
    rgb
}

fn write_png(path: &Path, rgb: &[u8]) {
    let side = (SLICE * SCALE) as u32;
    let file = File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut encoder = png::Encoder::new(BufWriter::new(file), side, side);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgb))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn saved_light_note(scene: &Scene) -> String {
    let relaxed = relaxed_block_light(scene);
    let (mut differ, mut cells, mut largest) = (0usize, 0usize, 0u8);
    for section in scene.inner() {
        let slot = scene.slot(section).unwrap();
        let (Some(saved), Some(relaxed)) = (&scene.light[slot], &relaxed[slot]) else {
            continue;
        };
        for (a, b) in saved.iter().zip(relaxed.iter()) {
            cells += 1;
            let d = a.abs_diff(*b);
            differ += (d > 0) as usize;
            largest = largest.max(d);
        }
    }
    format!(
        "{}: differs in {differ} of {cells} cells, by at most {largest}",
        scene.name
    )
}

fn percentile(values: &mut [Duration], q: f64) -> Duration {
    if values.is_empty() {
        return Duration::ZERO;
    }
    values.sort_unstable();
    let rank = ((values.len() as f64 * q).ceil() as usize).clamp(1, values.len());
    values[rank - 1]
}

fn ms(d: Duration) -> String {
    format!("{:.3} ms", d.as_secs_f64() * 1e3)
}
