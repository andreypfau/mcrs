use mcrs_minecraft_keys as keys;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use mcrs_minecraft_anvil::{Chunk, ChunkStatus, PaletteLookup, Properties, RegionFile};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_chunk::{PalettedContainer, VoxelId};
use mcrs_minecraft_core::{ColumnPos, RegionPos, SectionPos};
use mcrs_minecraft_light_color_bench::corpus::Corpus;
use mcrs_minecraft_light_color_bench::fixture::{
    Fixture, FixtureSection, SECTIONS, SIDE, relaxed_block_light,
};
use mcrs_minecraft_worldgen_testing::assets_dir;
use serde::Deserialize;

const USAGE: &str = "\
usage: cut_fixture scan <world> <dimension> [--max-y Y] [--by types|emitters]
       cut_fixture cut <world> <dimension> <x> <y> <z> <out>
       cut_fixture overlap <out>";

const WIDTH: usize = SectionPos::SIZE;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["scan", world, dimension, rest @ ..] => {
            let (mut max_y, mut by_emitters) = (None, false);
            let mut rest = rest.iter();
            while let Some(flag) = rest.next() {
                match (*flag, rest.next()) {
                    ("--max-y", Some(y)) => max_y = Some(number(y)),
                    ("--by", Some(&"types")) => by_emitters = false,
                    ("--by", Some(&"emitters")) => by_emitters = true,
                    _ => usage(),
                }
            }
            scan(Path::new(world), dimension, max_y, by_emitters);
        }
        ["cut", world, dimension, x, y, z, out] => {
            let centre = SectionPos::new(number(x), number(y), number(z));
            cut(Path::new(world), dimension, centre, Path::new(out));
        }
        ["overlap", out] => overlap(Path::new(out)),
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn number(text: &str) -> i32 {
    text.parse().unwrap_or_else(|_| usage())
}

/// Saved states resolve strictly against the corpus: a name it lacks fails the
/// read instead of turning into air.
struct CorpusStates;

impl PaletteLookup<VoxelId> for CorpusStates {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<VoxelId> {
        let block = Corpus::get().blocks.block(name)?;
        let mut id = block.default_state_id;
        for (property, text) in properties.iter() {
            id = block.with_text(id, property, text)?;
        }
        Some(VoxelId(id.0))
    }
}

struct AnyBiome;

impl PaletteLookup<u8> for AnyBiome {
    fn resolve(&self, _name: &str, _properties: Properties<'_>) -> Option<u8> {
        Some(0)
    }
}

struct World {
    dir: PathBuf,
    regions: HashMap<RegionPos, Option<RegionFile>>,
}

impl World {
    fn open(world: &Path, dimension: &str) -> World {
        let (namespace, path) = dimension.split_once(':').unwrap_or_else(|| usage());
        let dir = world
            .join("dimensions")
            .join(namespace)
            .join(path)
            .join("region");
        assert!(dir.is_dir(), "{} is not a directory", dir.display());
        World {
            dir,
            regions: HashMap::new(),
        }
    }

    fn region_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .unwrap_or_else(|e| panic!("{}: {e}", self.dir.display()))
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "mca"))
            .collect();
        files.sort();
        files
    }

    fn column(&mut self, pos: ColumnPos) -> Option<Chunk> {
        let region_pos = RegionPos::from(pos);
        let dir = &self.dir;
        let region = self.regions.entry(region_pos).or_insert_with(|| {
            let path = dir.join(format!("r.{}.{}.mca", region_pos.x, region_pos.z));
            path.exists()
                .then(|| RegionFile::open(&path).unwrap_or_else(|e| panic!("{e}")))
        });
        full_chunk(region.as_ref()?, pos)
    }
}

/// Only a full chunk holds the blocks and light its column is made of.
fn full_chunk(region: &RegionFile, pos: ColumnPos) -> Option<Chunk> {
    let chunk = region
        .read_chunk(pos, &CorpusStates, &AnyBiome)
        .unwrap_or_else(|e| panic!("{e}"))?;
    (chunk.status == ChunkStatus::Full).then_some(chunk)
}

fn emits(id: VoxelId) -> bool {
    !Corpus::get().registry.emission(id).is_zero()
}

fn type_bit(id: VoxelId) -> u32 {
    1 << Corpus::get().colours.light_type(id).0
}

/// Light types present as a bitmask, and the number of emitting cells.
fn emitters(states: &PalettedContainer<VoxelId, WIDTH>) -> (u32, u32) {
    match states {
        PalettedContainer::Homogeneous(id) if emits(*id) => (type_bit(*id), 4096),
        PalettedContainer::Homogeneous(_) => (0, 0),
        PalettedContainer::Heterogeneous(data) => data
            .palette
            .iter()
            .zip(&data.counts)
            .filter(|&(&id, &count)| count > 0 && emits(id))
            .fold((0, 0), |(mask, total), (&id, &count)| {
                (mask | type_bit(id), total + count as u32)
            }),
    }
}

fn scan(world: &Path, dimension: &str, max_y: Option<i32>, by_emitters: bool) {
    assert!(
        Corpus::get().colours.type_count() <= 32,
        "the scan keeps light types in a 32-bit mask"
    );
    let below = |sy: i32| max_y.is_none_or(|max| sy * WIDTH as i32 + WIDTH as i32 <= max);

    let world = World::open(world, dimension);
    let mut saved = HashSet::new();
    let mut lit: HashMap<SectionPos, (u32, u32)> = HashMap::new();
    for path in world.region_files() {
        let region = RegionFile::open(&path).unwrap_or_else(|e| panic!("{e}"));
        for column in region.present() {
            let Some(chunk) = full_chunk(&region, column) else {
                continue;
            };
            for section in &chunk.sections {
                let Some(states) = &section.block_states else {
                    continue;
                };
                let pos = SectionPos::new(column.x, section.y as i32, column.z);
                saved.insert(pos);
                let (mask, count) = emitters(states);
                if count > 0 && below(pos.y) {
                    lit.insert(pos, (mask, count));
                }
            }
        }
    }

    let extent = Extent::of(dimension);
    let in_world =
        |sy: i32| sy >= extent.min_section_y() && sy < extent.min_section_y() + extent.sections();
    let complete = |centre: SectionPos| {
        neighbours(centre, SIDE / 2).all(|pos| !in_world(pos.y) || saved.contains(&pos))
    };

    let centres: HashSet<SectionPos> = lit.keys().flat_map(|&pos| neighbours(pos, 1)).collect();
    let mut ranked: Vec<(SectionPos, u32, u32)> = centres
        .into_iter()
        .filter(|centre| below(centre.y))
        .map(|centre| {
            let (mask, count) = neighbours(centre, 1)
                .filter_map(|pos| lit.get(&pos))
                .fold((0, 0), |(m, c), &(mask, count)| (m | mask, c + count));
            (centre, mask.count_ones(), count)
        })
        .filter(|&(_, types, _)| !by_emitters || types >= 2)
        .collect();
    ranked.sort_by_key(|&(centre, types, count)| {
        let key = if by_emitters {
            (count, types)
        } else {
            (types, count)
        };
        (std::cmp::Reverse(key), centre.x, centre.y, centre.z)
    });

    println!(
        "{} saved sections, {} with an emitter",
        saved.len(),
        lit.len()
    );
    let mut shown = 0;
    for &(centre, types, count) in &ranked {
        if !complete(centre) {
            continue;
        }
        println!(
            "centre {} {} {}: {types} types, {count} emitters in 3x3x3",
            centre.x, centre.y, centre.z
        );
        shown += 1;
        if shown == 10 {
            break;
        }
    }
}

fn neighbours(centre: SectionPos, radius: i32) -> impl Iterator<Item = SectionPos> {
    (-radius..=radius).flat_map(move |dy| {
        (-radius..=radius).flat_map(move |dz| {
            (-radius..=radius)
                .map(move |dx| SectionPos::new(centre.x + dx, centre.y + dy, centre.z + dz))
        })
    })
}

#[derive(Deserialize)]
struct Extent {
    min_y: i32,
    height: u32,
}

impl Extent {
    fn of(dimension: &str) -> Extent {
        let (namespace, path) = dimension.split_once(':').unwrap_or_else(|| usage());
        let file = assets_dir()
            .join(namespace)
            .join("dimension_type")
            .join(format!("{path}.json"));
        let text =
            std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
    }

    fn min_section_y(&self) -> i32 {
        self.min_y.div_euclid(WIDTH as i32)
    }

    fn sections(&self) -> i32 {
        self.height as i32 / WIDTH as i32
    }
}

struct Cut {
    cells: Vec<VoxelId>,
    block_light: Option<Vec<u8>>,
}

fn cut(world: &Path, dimension: &str, centre: SectionPos, out: &Path) {
    let extent = Extent::of(dimension);
    let (min, count) = (extent.min_section_y(), extent.sections());
    let origin = SectionPos::new(
        centre.x - SIDE / 2,
        centre.y - SIDE / 2,
        centre.z - SIDE / 2,
    );
    let mut world = World::open(world, dimension);
    let mut columns: HashMap<ColumnPos, Option<Chunk>> = HashMap::new();

    let cuts: Vec<Option<Cut>> = (0..SECTIONS as i32)
        .map(|slot| {
            let pos = SectionPos::new(
                origin.x + slot % SIDE,
                origin.y + slot / (SIDE * SIDE),
                origin.z + slot / SIDE % SIDE,
            );
            if pos.y < min || pos.y >= min + count {
                return None;
            }
            let column = ColumnPos { x: pos.x, z: pos.z };
            let chunk = columns
                .entry(column)
                .or_insert_with(|| world.column(column))
                .as_ref()?;
            let section = chunk.sections.iter().find(|s| s.y as i32 == pos.y)?;
            let states = section.block_states.as_ref()?;
            let mut cells = Vec::with_capacity(SectionPos::VOLUME);
            for y in 0..WIDTH {
                for z in 0..WIDTH {
                    for x in 0..WIDTH {
                        cells.push(states.get(x, y, z));
                    }
                }
            }
            Some(Cut {
                cells,
                block_light: section.block_light.as_ref().map(|light| light.0.to_vec()),
            })
        })
        .collect();

    let used: Vec<VoxelId> = cuts
        .iter()
        .flatten()
        .flat_map(|cut| cut.cells.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let index: HashMap<VoxelId, u16> = used
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i as u16))
        .collect();
    let (colours, palette) = Corpus::get().bake(&used);

    let fixture = Fixture {
        dimension: dimension.to_owned(),
        min_section_y: min,
        section_count: count as u32,
        origin: [origin.x, origin.y, origin.z],
        colours,
        palette,
        sections: cuts
            .into_iter()
            .map(|cut| {
                cut.map(|cut| FixtureSection {
                    blocks: cut.cells.iter().map(|id| index[id]).collect(),
                    block_light: cut.block_light,
                })
            })
            .collect(),
    };
    fixture.write(out);

    let saved = fixture.sections.iter().flatten().count();
    let types: BTreeSet<u8> = fixture
        .palette
        .iter()
        .filter(|state| state.emission > 0)
        .map(|state| state.light_type)
        .collect();
    println!(
        "{}: centre {} {} {}, {saved} of {SECTIONS} sections saved, {} states, light types {:?}",
        out.display(),
        centre.x,
        centre.y,
        centre.z,
        fixture.palette.len(),
        types
    );
}

const OVERLAP_LIGHTS: [&str; 5] = [
    Block::Torch.as_static_str(),
    Block::SoulLantern.as_static_str(),
    Block::RedstoneTorch.as_static_str(),
    Block::AmethystCluster.as_static_str(),
    Block::CopperLantern.as_static_str(),
];

/// Five differently coloured lights on a ring of radius 4 on a stone floor
/// in open air, so every pair's light overlaps. The block light stored is the
/// server's relax over all of them together.
fn overlap(out: &Path) {
    let corpus = Corpus::get();
    let state = |name: &str| corpus.resolve(&name.parse().expect("a well-formed state"));
    let mut used = vec![
        state(Block::Air.as_static_str()),
        state(Block::Stone.as_static_str()),
    ];
    used.extend(OVERLAP_LIGHTS.map(state));
    let (colours, palette) = corpus.bake(&used);

    let centre = SectionPos::new(0, 4, 0);
    let origin = SectionPos::new(
        centre.x - SIDE / 2,
        centre.y - SIDE / 2,
        centre.z - SIDE / 2,
    );
    let floor_y = centre.y * WIDTH as i32;
    let middle = centre.x * WIDTH as i32 + WIDTH as i32 / 2;
    let ring: Vec<(i32, i32)> = (0..OVERLAP_LIGHTS.len())
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / OVERLAP_LIGHTS.len() as f32;
            (
                middle + (4.0 * angle.cos()).round() as i32,
                middle + (4.0 * angle.sin()).round() as i32,
            )
        })
        .collect();
    let inner = (origin.x + 1) * WIDTH as i32..(origin.x + SIDE - 1) * WIDTH as i32;

    let sections = (0..SECTIONS as i32)
        .map(|slot| {
            let pos = SectionPos::new(
                origin.x + slot % SIDE,
                origin.y + slot / (SIDE * SIDE),
                origin.z + slot / SIDE % SIDE,
            );
            let mut blocks = vec![0u16; SectionPos::VOLUME];
            for (i, block) in blocks.iter_mut().enumerate() {
                let x = pos.x * WIDTH as i32 + (i % WIDTH) as i32;
                let z = pos.z * WIDTH as i32 + (i / WIDTH % WIDTH) as i32;
                let y = pos.y * WIDTH as i32 + (i / (WIDTH * WIDTH)) as i32;
                if y == floor_y && inner.contains(&x) && inner.contains(&z) {
                    *block = 1;
                } else if y == floor_y + 1
                    && let Some(light) = ring.iter().position(|&(rx, rz)| (rx, rz) == (x, z))
                {
                    *block = 2 + light as u16;
                }
            }
            Some(FixtureSection {
                blocks,
                block_light: None,
            })
        })
        .collect();

    let extent = Extent::of(mcrs_minecraft_dimension::keys::dimension::OVERWORLD.as_str());
    let mut fixture = Fixture {
        dimension: mcrs_minecraft_dimension::keys::dimension::OVERWORLD
            .as_str()
            .to_owned(),
        min_section_y: extent.min_section_y(),
        section_count: extent.sections() as u32,
        origin: [origin.x, origin.y, origin.z],
        colours,
        palette,
        sections,
    };
    let light = relaxed_block_light(&fixture.scene("overlap"));
    for (section, light) in fixture.sections.iter_mut().zip(light) {
        let (Some(section), Some(light)) = (section, light) else {
            continue;
        };
        let mut packed = vec![0u8; SectionPos::VOLUME / 2];
        for (i, &level) in light.iter().enumerate() {
            packed[i >> 1] |= level << ((i & 1) * 4);
        }
        section.block_light = Some(packed);
    }
    fixture.write(out);
    println!(
        "{}: lights at {ring:?}, floor at y {floor_y}",
        out.display()
    );
}
