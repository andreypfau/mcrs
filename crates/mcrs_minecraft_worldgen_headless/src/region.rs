#![expect(
    clippy::too_many_lines,
    clippy::excessive_nesting,
    reason = "one pass per map over the same grids"
)]
#![expect(
    clippy::indexing_slicing,
    reason = "grid code indexes buffers it sized itself"
)]

use std::collections::HashMap;
use std::time::Instant;

use glam::IVec3;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    CONTINENTS, DEPTH, EROSION, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen_generator::staging::FilledSnapshot;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos};
use rayon::prelude::*;

use crate::World;

#[derive(Clone, Copy, Debug)]
pub struct SliceSpec {
    pub along_x: bool,
    pub at: i32,
}

pub struct Slice {
    pub spec: SliceSpec,
    pub cells: Vec<u32>,
    pub len: usize,
}

pub struct Region {
    pub x0: i32,
    pub z0: i32,
    pub w: usize,
    pub d: usize,
    pub min_y: i32,
    pub height: usize,
    pub top: Vec<i32>,
    pub ground: Vec<i32>,
    pub top_block: Vec<u32>,
    pub ground_block: Vec<u32>,
    pub biome: Vec<u8>,
    pub shallow_air: Vec<u16>,
    pub volume: Vec<u64>,
    pub slices: Vec<Slice>,
    pub names: Vec<String>,
    pub wall_ms: u128,
    pub columns: usize,
    pub capture: Option<Capture>,
}

pub struct Capture {
    pub y0: i32,
    pub height: usize,
    pub states: Vec<u16>,
}

impl Region {
    pub fn at(&self, x: usize, z: usize) -> usize {
        z * self.w + x
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Air,
    Fluid,
    Plant,
    Solid,
}

pub fn class_of(name: &str) -> Class {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    if matches!(name, "air" | "cave_air" | "void_air") {
        return Class::Air;
    }
    if matches!(
        name,
        "water" | "lava" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant" | "bubble_column"
    ) {
        return Class::Fluid;
    }
    const PLANT: [&str; 22] = [
        "leaves",
        "_log",
        "_wood",
        "short_grass",
        "tall_grass",
        "fern",
        "flower",
        "bush",
        "sapling",
        "vine",
        "mushroom",
        "petals",
        "litter",
        "sugar_cane",
        "bamboo",
        "cactus",
        "berry",
        "dripleaf",
        "azalea",
        "carpet",
        "lily",
        "snow",
    ];
    const FLOWERS: [&str; 10] = [
        "poppy",
        "dandelion",
        "tulip",
        "orchid",
        "allium",
        "bluet",
        "daisy",
        "cornflower",
        "lily_of_the_valley",
        "torchflower",
    ];
    let solid_lookalike = matches!(name, "snow_block" | "mushroom_stem" | "moss_block")
        || name.ends_with("mushroom_block");
    if !solid_lookalike
        && (PLANT.iter().any(|p| name.contains(p)) || FLOWERS.iter().any(|p| name.contains(p)))
    {
        return Class::Plant;
    }
    Class::Solid
}

struct Names {
    by_voxel: HashMap<u32, u32>,
    names: Vec<String>,
    classes: Vec<Class>,
}

impl Names {
    fn new() -> Self {
        Names {
            by_voxel: HashMap::new(),
            names: Vec::new(),
            classes: Vec::new(),
        }
    }

    fn index(&mut self, voxel: u32) -> u32 {
        if let Some(&index) = self.by_voxel.get(&voxel) {
            return index;
        }
        let corpus = &mcrs_minecraft_worldgen_generator::tests::blocks().0;
        let name = BlockStateId::checked(voxel as _, corpus.state_count())
            .and_then(|state| corpus.owner(state))
            .map_or_else(|| format!("#{voxel}"), |block| block.identifier.to_string());
        let index = self.names.len() as u32;
        self.classes.push(class_of(&name));
        self.names.push(name);
        self.by_voxel.insert(voxel, index);
        index
    }
}

struct ColumnOut {
    col: ColumnPos,
    top: Vec<i32>,
    ground: Vec<i32>,
    top_block: Vec<u32>,
    ground_block: Vec<u32>,
    biome: Vec<u8>,
    shallow_air: Vec<u16>,
    volume: Vec<u64>,
    cuts: Vec<(usize, usize, Vec<u32>)>,
    states: Vec<u16>,
}

fn voxels(snap: &FilledSnapshot, height: usize) -> Vec<u32> {
    let mut out = vec![0u32; 256 * height];
    for (si, payload) in snap.sections.iter().enumerate() {
        let Some((blocks, _)) = payload.as_ref() else {
            continue;
        };
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let id = blocks.get(LocalPos::from(BlockPos::new(x, y, z)));
                    let gy = si * 16 + y as usize;
                    out[(gy * 16 + z as usize) * 16 + x as usize] = u32::from(id.0);
                }
            }
        }
    }
    out
}

fn summarize(
    snap: &FilledSnapshot,
    names: &mut Names,
    min_y: i32,
    height: usize,
    slices: &[SliceSpec],
    capture: Option<(usize, usize)>,
) -> ColumnOut {
    let raw = voxels(snap, height);
    let cell = |x: usize, y: usize, z: usize| raw[(y * 16 + z) * 16 + x];
    let mut out = ColumnOut {
        col: snap.col,
        top: vec![min_y; 256],
        ground: vec![min_y; 256],
        top_block: vec![0; 256],
        ground_block: vec![0; 256],
        biome: vec![0; 256],
        shallow_air: vec![0; 256],
        volume: vec![0; 256],
        cuts: Vec::new(),
        states: capture.map_or_else(Vec::new, |(from, h)| {
            raw[from * 256..(from + h) * 256]
                .iter()
                .map(|&v| v as u16)
                .collect()
        }),
    };
    for (_, biomes) in snap.sections.iter().flatten() {
        for y in (0..16).step_by(4) {
            for z in (0..16).step_by(4) {
                for x in (0..16).step_by(4) {
                    out.volume[usize::from(biomes.0.get(x, y, z).unwrap_or(0))] += 1;
                }
            }
        }
    }
    for z in 0..16 {
        for x in 0..16 {
            let i = z * 16 + x;
            let mut found_top = false;
            for y in (0..height).rev() {
                let index = names.index(cell(x, y, z));
                let class = names.classes[index as usize];
                if class == Class::Air {
                    continue;
                }
                if !found_top {
                    found_top = true;
                    out.top[i] = min_y + y as i32;
                    out.top_block[i] = index;
                }
                if class == Class::Solid {
                    out.ground[i] = min_y + y as i32;
                    out.ground_block[i] = index;
                    let mut air = 0u16;
                    for below in y.saturating_sub(30)..y {
                        let index = names.index(cell(x, below, z));
                        if names.classes[index as usize] == Class::Air {
                            air += 1;
                        }
                    }
                    out.shallow_air[i] = air;
                    let section = y / 16;
                    out.biome[i] = snap
                        .sections
                        .get(section)
                        .and_then(Option::as_ref)
                        .and_then(|(_, biomes)| biomes.0.get(x, y % 16, z))
                        .unwrap_or(0);
                    break;
                }
            }
        }
    }
    let (bx, bz) = (snap.col.x * 16, snap.col.z * 16);
    for (s, spec) in slices.iter().enumerate() {
        let local = spec.at - if spec.along_x { bz } else { bx };
        if !(0..16).contains(&local) {
            continue;
        }
        for k in 0..16usize {
            let (x, z) = if spec.along_x {
                (k, local as usize)
            } else {
                (local as usize, k)
            };
            let cells = (0..height).map(|y| names.index(cell(x, y, z))).collect();
            out.cuts.push((s, k, cells));
        }
    }
    out
}

pub fn generate(
    world: &World,
    (cx0, cz0, cx1, cz1): (i32, i32, i32, i32),
    tile: i32,
    slices: &[SliceSpec],
    capture: Option<(i32, i32)>,
) -> Result<Region, String> {
    if tile <= 0 {
        return Err(format!("the tile must be positive, got {tile}"));
    }
    if cx1 < cx0 || cz1 < cz0 {
        return Err(format!(
            "the area runs backwards: chunks {cx0},{cz0} to {cx1},{cz1}"
        ));
    }
    let ctx = &world.ctx;
    let first = ctx.y_sections.first().ok_or("the world has no sections")?;
    let min_y = first * 16;
    let height = ctx.y_sections.len() * 16;
    let band = capture.map(|(y0, y1)| {
        let from = (y0 - min_y).clamp(0, height as i32 - 1) as usize;
        let to = (y1 - min_y).clamp(from as i32, height as i32 - 1) as usize;
        (from, to - from + 1)
    });
    let mut tiles = Vec::new();
    let mut tz = cz0;
    while tz <= cz1 {
        let mut tx = cx0;
        while tx <= cx1 {
            tiles.push((tx, tz, (tx + tile - 1).min(cx1), (tz + tile - 1).min(cz1)));
            tx += tile;
        }
        tz += tile;
    }
    let started = Instant::now();
    let parts: Vec<(Vec<ColumnOut>, Names)> = tiles
        .par_iter()
        .map(|&(x0, z0, x1, z1)| {
            let snaps = mcrs_minecraft_worldgen_generator::tests::generate_region(
                ctx,
                ColumnPos::new(x0, z0),
                ColumnPos::new(x1, z1),
            );
            let mut names = Names::new();
            let outs = snaps
                .values()
                .map(|snap| summarize(snap, &mut names, min_y, height, slices, band))
                .collect();
            (outs, names)
        })
        .collect();
    let wall_ms = started.elapsed().as_millis();

    let (w, d) = (
        ((cx1 - cx0 + 1) * 16) as usize,
        ((cz1 - cz0 + 1) * 16) as usize,
    );
    let mut region = Region {
        x0: cx0 * 16,
        z0: cz0 * 16,
        w,
        d,
        min_y,
        height,
        top: vec![min_y; w * d],
        ground: vec![min_y; w * d],
        top_block: vec![0; w * d],
        ground_block: vec![0; w * d],
        biome: vec![0; w * d],
        shallow_air: vec![0; w * d],
        volume: vec![0; 256],
        slices: slices
            .iter()
            .map(|&spec| {
                let len = if spec.along_x { w } else { d };
                Slice {
                    spec,
                    cells: vec![0; len * height],
                    len,
                }
            })
            .collect(),
        names: Vec::new(),
        wall_ms,
        columns: 0,
        capture: band.map(|(from, h)| Capture {
            y0: min_y + from as i32,
            height: h,
            states: vec![0; w * d * h],
        }),
    };
    let mut global = Names::new();
    for (outs, names) in parts {
        let remap: Vec<u32> = names
            .names
            .iter()
            .map(|name| {
                let at = global.names.iter().position(|n| n == name);
                at.map_or_else(
                    || {
                        global.names.push(name.clone());
                        global.classes.push(class_of(name));
                        (global.names.len() - 1) as u32
                    },
                    |at| at as u32,
                )
            })
            .collect();
        for out in outs {
            region.columns += 1;
            for (sum, n) in region.volume.iter_mut().zip(&out.volume) {
                *sum += n;
            }
            let ox = (out.col.x - cx0) as usize * 16;
            let oz = (out.col.z - cz0) as usize * 16;
            for z in 0..16 {
                for x in 0..16 {
                    let (i, at) = (z * 16 + x, region.at(ox + x, oz + z));
                    region.top[at] = out.top[i];
                    region.ground[at] = out.ground[i];
                    region.top_block[at] = remap[out.top_block[i] as usize];
                    region.ground_block[at] = remap[out.ground_block[i] as usize];
                    region.biome[at] = out.biome[i];
                    region.shallow_air[at] = out.shallow_air[i];
                }
            }
            if let Some(capture) = region.capture.as_mut() {
                for (i, &state) in out.states.iter().enumerate() {
                    let (x, z, y) = (i % 16, (i / 16) % 16, i / 256);
                    capture.states[(y * d + oz + z) * w + ox + x] = state;
                }
            }
            for (s, k, cells) in out.cuts {
                let slice = &mut region.slices[s];
                let pos = if slice.spec.along_x { ox + k } else { oz + k };
                for (y, cell) in cells.into_iter().enumerate() {
                    slice.cells[y * slice.len + pos] = remap[cell as usize];
                }
            }
        }
    }
    region.names = global.names;
    Ok(region)
}

pub fn climate(
    world: &World,
    (x0, z0): (i32, i32),
    (nx, nz): (i32, i32),
    step: i32,
    y: i32,
) -> [Vec<f32>; 6] {
    if nx <= 0 || nz <= 0 || step <= 0 {
        return std::array::from_fn(|_| Vec::new());
    }
    let roots = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
    let volume = SampleGrid::new(
        IVec3::new(nx, 1, nz),
        IVec3::new(x0, y, z0),
        IVec3::splat(step),
    );
    let points = volume.len();
    let mut values = vec![0.0f32; roots.len() * points];
    if let Some(router) = world.ctx.router.as_ref() {
        router.fill_roots(&mut Workspace::new(), &volume, &roots, &mut values);
    }
    std::array::from_fn(|r| values[r * points..(r + 1) * points].to_vec())
}
