use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};

use crate::anvil::{
    self, Palette, REGION_BLOCKS, REGION_CHUNKS, Region, SECTION_SIZE, Window, World,
};
use crate::arena::{Arena, Block};
use crate::blocks::{self, BlockInfo, Catalog};
use crate::cave::CaveCull;
use crate::mesh::{self, CONNECT_ALL, Draw, Group, STREAM_NAMES, STREAMS, Scratch, SectionMesh};
use crate::pack::{
    QUAD_WORDS, RENDER_REGION_X, RENDER_REGION_Y, RENDER_REGION_Z, RegionGrid, SECTION_FACE_TABLE,
    SECTIONS_PER_RENDER_REGION,
};
use crate::render::{Animation, Atlas, Layout, Placement, Upload, Uploads};

const HYSTERESIS: f32 = (RENDER_REGION_X * SECTION_SIZE) as f32;

const FILES_IN_FLIGHT: usize = 4;

const SECTIONS_IN_FLIGHT: usize = 128;

const SECTIONS_PER_FRAME: usize = 32;

#[derive(Resource)]
pub struct Loader {
    layout: Arc<Layout>,
    uploads: Uploads,
    palette: Palette,
    catalog: Option<Catalog>,
    world: Arc<World>,
    newest: Arc<World>,
    blocks: Arc<Vec<BlockInfo>>,
    expected: HashSet<[i32; 2]>,
    to_parse: Vec<([i32; 2], PathBuf)>,
    parsing: Vec<([i32; 2], Task<Result<Region, String>>)>,
    tinting: Vec<([u32; 2], Task<Vec<u8>>)>,
    baking: Option<(Arc<World>, Vec<[i32; 2]>, Task<Baked>)>,
    to_tint: Vec<[i32; 2]>,
    resident: Vec<Option<Resident>>,
    deferred: Vec<bool>,
    pending: Vec<bool>,
    meshing: Vec<(usize, Task<SectionMesh>)>,
    regions: Vec<RegionSlot>,
    dirty: Vec<bool>,
    camera: Vec3,
    anchor: Vec3,
    evicted: usize,
    quads: Arena,
    models: Arena,
    faces: Arena,
    groups: Arena,
    dropped: usize,
    sprites: usize,
    started: Instant,
    reported: bool,
}

struct Resident {
    quads: Block,
    models: Block,
    faces: Block,
    connectivity: u64,
}

/// The render region owns the group records and the face prefix table of every section placed
/// in it, because a draw covers one bucket of one region and the cull walks that span.
struct RegionSlot {
    lists: [Vec<(u32, Group)>; STREAMS],
    groups: Block,
    table: Block,
    faces: Vec<u32>,
    live: usize,
}

impl RegionSlot {
    fn new() -> Self {
        Self {
            lists: std::array::from_fn(|_| Vec::new()),
            groups: Block::EMPTY,
            table: Block::EMPTY,
            faces: Vec::new(),
            live: 0,
        }
    }
}

struct Baked {
    catalog: Catalog,
    blocks: Vec<BlockInfo>,
    sprites: Option<Upload>,
}

pub struct Status {
    pub files: usize,
    pub files_total: usize,
    pub sections: usize,
    pub sections_total: usize,
    pub evicted: usize,
    pub quads: f32,
    pub models: f32,
}

impl Loader {
    pub fn new(layout: Arc<Layout>, uploads: Uploads, window: Window) -> Self {
        let world = World::new(window.min_region, window.regions);
        let (layout_quads, layout_models, layout_faces, layout_groups) = (
            layout.quad_capacity,
            layout.model_capacity,
            layout.face_capacity,
            layout.group_capacity,
        );
        let regions = layout.grid.len();
        let slots = layout.grid.slots();
        Self {
            expected: window.files.iter().map(|(coords, _)| *coords).collect(),
            to_parse: window.files,
            layout,
            uploads,
            palette: Palette::new(),
            catalog: Some(blocks::empty()),
            world: Arc::new(world.clone()),
            newest: Arc::new(world),
            blocks: Arc::new(Vec::new()),
            parsing: Vec::new(),
            baking: None,
            to_tint: Vec::new(),
            tinting: Vec::new(),
            resident: (0..slots).map(|_| None).collect(),
            deferred: vec![false; slots],
            pending: vec![false; slots],
            meshing: Vec::new(),
            regions: (0..regions).map(|_| RegionSlot::new()).collect(),
            dirty: vec![false; regions],
            camera: Vec3::ZERO,
            anchor: Vec3::splat(f32::MAX),
            evicted: 0,
            quads: Arena::new(layout_quads),
            models: Arena::new(layout_models),
            faces: Arena::new(layout_faces),
            groups: Arena::new(layout_groups),
            dropped: 0,
            sprites: 0,
            started: Instant::now(),
            reported: false,
        }
    }

    pub fn status(&self) -> Status {
        Status {
            files: self.world.loaded(),
            files_total: self.expected.len(),
            sections: self.regions.iter().map(|region| region.live).sum(),
            sections_total: self.world.non_empty_sections(),
            evicted: self.evicted,
            quads: self.quads.held() as f32 / self.quads.capacity() as f32,
            models: self.models.held() as f32 / self.models.capacity() as f32,
        }
    }

    pub fn done(&self) -> bool {
        self.reported && self.uploads.waiting() == 0
    }

    fn idle(&self) -> bool {
        self.to_parse.is_empty()
            && self.parsing.is_empty()
            && self.baking.is_none()
            && self.to_tint.is_empty()
            && self.tinting.is_empty()
            && self.meshing.is_empty()
            && self.dirty.iter().all(|dirty| !dirty)
            && self.wanted(1).is_empty()
    }

    fn ready_to_mesh(&self, region: usize) -> bool {
        files_read(self.layout.grid, self.world.min_region, region)
            .iter()
            .all(|coords| !self.expected.contains(coords) || self.world.holds(*coords))
    }

    fn region_distance(&self, region: usize) -> f32 {
        let origin = self.layout.grid.origin(self.layout.min_section, region);
        let min = Vec3::new(origin[0] as f32, origin[1] as f32, origin[2] as f32);
        let max = min
            + Vec3::new(
                (RENDER_REGION_X * SECTION_SIZE) as f32,
                (RENDER_REGION_Y * SECTION_SIZE) as f32,
                (RENDER_REGION_Z * SECTION_SIZE) as f32,
            );
        (self.camera.clamp(min, max) - self.camera).length()
    }

    fn distance(&self, slot: usize) -> f32 {
        let section = self.layout.grid.section_at(slot);
        let corner = std::array::from_fn(|axis| {
            ((section[axis] as i32 + self.layout.min_section[axis]) * SECTION_SIZE as i32) as f32
        });
        let min = Vec3::from_array(corner);
        let max = min + Vec3::splat(SECTION_SIZE as f32);
        (self.camera.clamp(min, max) - self.camera).length()
    }

    /// The nearest sections that hold blocks, have nowhere to be but the arena, and whose region
    /// has every file it borders. Nothing more than distance decides the order yet.
    fn wanted(&self, want: usize) -> Vec<usize> {
        if want == 0 {
            return Vec::new();
        }
        let grid = self.layout.grid;
        let ready: Vec<bool> = (0..grid.len())
            .map(|region| self.ready_to_mesh(region))
            .collect();
        let mut nearest: Vec<(f32, usize)> = Vec::new();
        for slot in 0..grid.slots() {
            if self.resident[slot].is_some() || self.deferred[slot] || self.pending[slot] {
                continue;
            }
            if !ready[slot / SECTIONS_PER_RENDER_REGION] {
                continue;
            }
            let [sx, sy, sz] = grid.section_at(slot);
            let inside = (0..3).all(|axis| [sx, sy, sz][axis] < self.world.sections[axis]);
            if !inside || self.world.section(sx, sy, sz).is_none() {
                continue;
            }
            nearest.push((self.distance(slot), slot));
        }
        if nearest.len() > want {
            nearest.select_nth_unstable_by(want, |a, b| a.0.total_cmp(&b.0));
            nearest.truncate(want);
        }
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        nearest.into_iter().map(|(_, slot)| slot).collect()
    }

    fn victim(&self, candidate: f32) -> Option<usize> {
        let region = (0..self.layout.grid.len())
            .filter(|region| self.regions[*region].live > 0)
            .max_by(|a, b| {
                self.region_distance(*a)
                    .total_cmp(&self.region_distance(*b))
            })?;
        let base = region * SECTIONS_PER_RENDER_REGION;
        let farthest = (base..base + SECTIONS_PER_RENDER_REGION)
            .filter(|slot| self.resident[*slot].is_some())
            .max_by(|a, b| self.distance(*a).total_cmp(&self.distance(*b)))?;
        worth_evicting(self.distance(farthest), candidate).then_some(farthest)
    }

    fn evict(&mut self, slot: usize, cave: &mut CaveCull) {
        let Some(held) = self.resident[slot].take() else {
            return;
        };
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.faces.free(held.faces);

        let region = slot / SECTIONS_PER_RENDER_REGION;
        let local = (slot % SECTIONS_PER_RENDER_REGION) as u32;
        let held = &mut self.regions[region];
        for list in &mut held.lists {
            list.retain(|(owner, _)| *owner != local);
        }
        held.faces[local as usize] = 0;
        held.live -= 1;

        if let Some(base) = self.cave_base(cave, region) {
            cave.set_section(base + local as usize, CONNECT_ALL);
        }
        self.dirty[region] = true;
        self.evicted += 1;
        self.deferred.fill(false);
    }

    fn cave_base(&self, cave: &CaveCull, region: usize) -> Option<usize> {
        let corner = self.layout.grid.corner(region);
        let grid = cave.grid();
        let extent = grid.extent();
        let mut local = [0usize; 3];
        for axis in 0..3 {
            let at = corner[axis] as i32 + self.layout.min_section[axis] - cave.min_section()[axis];
            if at < 0 || at as usize >= extent[axis] {
                return None;
            }
            local[axis] = at as usize;
        }
        Some(grid.split(local[0], local[1], local[2]).0 * SECTIONS_PER_RENDER_REGION)
    }

    fn retarget(&mut self, cave: &mut CaveCull) {
        let grid = cave.grid();
        let span = grid.extent();
        let extent = [span[0] as i32, 0, span[2] as i32];
        let window = [
            (self.world.regions[0] * REGION_CHUNKS) as i32,
            0,
            (self.world.regions[1] * REGION_CHUNKS) as i32,
        ];
        let mut corner = [0i32; 3];
        for axis in [0, 2] {
            let camera = if axis == 0 {
                self.camera.x
            } else {
                self.camera.z
            };
            let here = (camera / (RENDER_REGION_X * SECTION_SIZE) as f32).floor() as i32;
            let centred = (here - grid.x as i32 / 2) * RENDER_REGION_X as i32;
            let low = self.layout.min_section[axis];
            let high = low + window[axis] - extent[axis];
            corner[axis] = centred.clamp(low, high.max(low));
        }
        corner[1] = crate::anvil::MIN_SECTION_Y;

        if corner == cave.min_section() {
            return;
        }
        cave.retarget(corner);

        let bases: Vec<Option<usize>> = (0..self.layout.grid.len())
            .map(|region| self.cave_base(cave, region))
            .collect();
        for (slot, held) in self.resident.iter().enumerate() {
            let Some(held) = held else { continue };
            if let Some(base) = bases[slot / SECTIONS_PER_RENDER_REGION] {
                cave.set_section(base + slot % SECTIONS_PER_RENDER_REGION, held.connectivity);
            }
        }
        self.uploads.rebase(
            bases
                .iter()
                .enumerate()
                .filter(|(region, _)| self.regions[*region].live > 0)
                .map(|(region, base)| {
                    (
                        region as u32,
                        base.map_or(cave.always_visible(), |base| base as u32),
                    )
                })
                .collect(),
        );
    }

    fn reserve(&mut self, mesh: &SectionMesh, region: usize) -> Option<[Block; 4]> {
        let table = match self.regions[region].faces.is_empty() {
            true => Some(self.faces.alloc(SECTION_FACE_TABLE)?),
            false => None,
        };
        let quads = self.quads.alloc(mesh.simple.len());
        let models = self.models.alloc(mesh.model_quads());
        let faces = self.faces.alloc(mesh.faces.len());
        match (quads, models, faces) {
            (Some(quads), Some(models), Some(faces)) => {
                Some([quads, models, faces, table.unwrap_or(Block::EMPTY)])
            }
            (quads, models, faces) => {
                if let Some(quads) = quads {
                    self.quads.free(quads);
                }
                if let Some(models) = models {
                    self.models.free(models);
                }
                if let Some(faces) = faces {
                    self.faces.free(faces);
                }
                if let Some(table) = table {
                    self.faces.free(table);
                }
                None
            }
        }
    }

    fn place(&mut self, mesh: SectionMesh, cave: &mut CaveCull) -> Result<Placement, SectionMesh> {
        let grid = self.layout.grid;
        let slot = grid.slot(mesh.section[0], mesh.section[1], mesh.section[2]);
        let region = slot / SECTIONS_PER_RENDER_REGION;
        let local = (slot % SECTIONS_PER_RENDER_REGION) as u32;

        let Some([quads, models, faces, table]) = self.reserve(&mesh, region) else {
            return Err(mesh);
        };

        let held = &mut self.regions[region];
        if held.faces.is_empty() {
            held.faces = vec![0; SECTION_FACE_TABLE];
            held.table = table;
        }
        let mut placed = mesh.groups;
        let mut first = 0usize;
        for stream in 0..STREAMS {
            let run = mesh.spans[stream].group_count as usize;
            let base = if stream % 2 == 0 {
                quads.offset
            } else {
                models.offset
            } as u32;
            for group in &mut placed[first..first + run] {
                group.quad_base += base;
            }
            held.lists[stream].extend(placed[first..first + run].iter().map(|g| (local, *g)));
            first += run;
        }
        held.faces[local as usize] = faces.offset as u32;
        held.live += 1;

        if let Some(base) = self.cave_base(cave, region) {
            cave.set_section(base + local as usize, mesh.connectivity);
        }
        self.resident[slot] = Some(Resident {
            quads,
            models,
            faces,
            connectivity: mesh.connectivity,
        });
        self.dirty[region] |= !placed.is_empty();

        Ok(Placement {
            quads: ((quads.offset * QUAD_WORDS * 4) as u64, mesh.simple),
            vertices: ((models.offset * 4 * 3 * 4) as u64, mesh.complex),
            faces: ((faces.offset * 4) as u64, mesh.faces),
            groups: (0, Vec::new()),
            draws: Vec::new(),
            replaces: None,
        })
    }

    /// A region hands its whole group block back and takes a fresh one, so the block it is being
    /// drawn from is never the block being written.
    fn flush(&mut self, region: usize, cave: &CaveCull) -> Option<Placement> {
        let total: usize = self.regions[region].lists.iter().map(Vec::len).sum();
        let block = self.groups.alloc(total)?;
        let stale = std::mem::replace(&mut self.regions[region].groups, block);
        self.groups.free(stale);

        let cave_base = self
            .cave_base(cave, region)
            .map_or(cave.always_visible(), |base| base as u32);
        let origin = self.layout.grid.origin(self.layout.min_section, region);
        let held = &self.regions[region];
        let mut records = Vec::with_capacity(total);
        let mut draws = Vec::new();
        for stream in 0..STREAMS {
            let first = records.len();
            let mut quads = 0u32;
            for (_, group) in &held.lists[stream] {
                let mut group = *group;
                group.quad_prefix = quads;
                quads += group.quad_count;
                records.push(group);
            }
            if records.len() == first {
                continue;
            }
            draws.push(Draw {
                stream: stream as u32,
                region: region as u32,
                origin,
                cave_base,
                face_base: held.table.offset as u32,
                first_group: (block.offset + first) as u32,
                group_count: (records.len() - first) as u32,
                quad_count: quads,
            });
        }
        Some(Placement {
            quads: (0, Vec::new()),
            vertices: (0, Vec::new()),
            faces: ((held.table.offset * 4) as u64, held.faces.clone()),
            groups: ((block.offset * size_of::<Group>()) as u64, records),
            draws,
            replaces: Some(region as u32),
        })
    }
}

fn worth_evicting(resident: f32, candidate: f32) -> bool {
    resident > candidate + HYSTERESIS
}

fn files_read(grid: RegionGrid, min_region: [i32; 2], region: usize) -> [[i32; 2]; 4] {
    let [sx, _, sz] = grid.corner(region);
    let size = SECTION_SIZE as i32;
    let span = REGION_BLOCKS as i32;
    let xs = [sx as i32 * size - 1, (sx + RENDER_REGION_X) as i32 * size];
    let zs = [sz as i32 * size - 1, (sz + RENDER_REGION_Z) as i32 * size];
    std::array::from_fn(|corner| {
        [
            min_region[0] + xs[corner & 1].div_euclid(span),
            min_region[1] + zs[corner >> 1].div_euclid(span),
        ]
    })
}

pub fn advance(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let pool = AsyncComputeTaskPool::get();
    let loader = &mut *loader;
    loader.camera = camera.translation();
    loader.retarget(&mut cave);
    if loader.camera.distance(loader.anchor) > HYSTERESIS {
        loader.anchor = loader.camera;
        loader.deferred.fill(false);
    }

    let mut parsed = Vec::new();
    loader
        .parsing
        .retain_mut(|(coords, task)| match check_ready(task) {
            Some(result) => {
                parsed.push((*coords, result));
                false
            }
            None => true,
        });
    for (coords, result) in parsed {
        match result {
            Ok(region) => absorb(loader, coords, region),
            Err(error) => {
                println!("skipping r.{}.{}: {error}", coords[0], coords[1]);
                loader.expected.remove(&coords);
            }
        }
    }

    if let Some((_, _, task)) = loader.baking.as_mut()
        && let Some(baked) = check_ready(task)
    {
        let (world, tinted) = loader
            .baking
            .take()
            .map(|(w, t, _)| (w, t))
            .expect("just held");
        publish(loader, world, tinted, Some(baked), pool);
    }
    settle(loader, pool);

    let mut tinted = Vec::new();
    loader
        .tinting
        .retain_mut(|(origin, task)| match check_ready(task) {
            Some(data) => {
                tinted.push((*origin, data));
                false
            }
            None => true,
        });
    for (origin, data) in tinted {
        loader.uploads.push(Upload::Tints {
            origin,
            size: REGION_BLOCKS as u32,
            data,
        });
    }

    let mut meshed = Vec::new();
    loader
        .meshing
        .retain_mut(|(slot, task)| match check_ready(task) {
            Some(mesh) => {
                meshed.push((*slot, mesh));
                false
            }
            None => true,
        });
    for (slot, mesh) in meshed {
        loader.pending[slot] = false;
        let here = loader.distance(slot);
        let mut pending = mesh;
        loop {
            match loader.place(pending, &mut cave) {
                Ok(placement) => {
                    if !placement.quads.1.is_empty() || !placement.vertices.1.is_empty() {
                        loader.uploads.push(Upload::Geometry(placement));
                    }
                    break;
                }
                Err(back) => match loader.victim(here) {
                    Some(victim) => {
                        loader.evict(victim, &mut cave);
                        pending = back;
                    }
                    None => {
                        loader.deferred[slot] = true;
                        break;
                    }
                },
            }
        }
    }

    for region in 0..loader.dirty.len() {
        if !loader.dirty[region] {
            continue;
        }
        if let Some(placement) = loader.flush(region, &cave) {
            loader.dirty[region] = false;
            loader.uploads.push(Upload::Geometry(placement));
        }
    }

    while loader.parsing.len() < FILES_IN_FLIGHT
        && let Some((coords, path)) = loader.to_parse.pop()
    {
        loader
            .parsing
            .push((coords, pool.spawn(async move { anvil::load(&path) })));
    }

    let room = SECTIONS_IN_FLIGHT.saturating_sub(loader.meshing.len());
    for slot in loader.wanted(room.min(SECTIONS_PER_FRAME)) {
        let world = loader.world.clone();
        let blocks = loader.blocks.clone();
        let grid = loader.layout.grid;
        let section = grid.section_at(slot);
        loader.pending[slot] = true;
        loader.meshing.push((
            slot,
            pool.spawn(async move {
                let mut scratch = Scratch::new();
                mesh::mesh_section(&world, &blocks, grid, section, &mut scratch)
            }),
        ));
    }

    if !loader.reported && loader.idle() {
        loader.reported = true;
        report(loader);
    }
}

fn absorb(loader: &mut Loader, coords: [i32; 2], region: Region) {
    let mut world = (*loader.newest).clone();
    loader.dropped += world.insert(&mut loader.palette, coords, region);
    loader.newest = Arc::new(world);
    loader.to_tint.push(coords);
}

fn settle(loader: &mut Loader, pool: &'static AsyncComputeTaskPool) {
    if loader.baking.is_some() {
        return;
    }
    let catalog = loader.catalog.as_ref().expect("nothing is baking");
    match next_step(
        catalog.blocks.len(),
        loader.palette.states.len(),
        Arc::ptr_eq(&loader.world, &loader.newest),
    ) {
        Next::Bake => start_baking(loader, pool),
        Next::Publish => {
            let world = loader.newest.clone();
            let tinted = std::mem::take(&mut loader.to_tint);
            publish(loader, world, tinted, None, pool);
        }
        Next::Wait => {}
    }
}

#[derive(PartialEq, Eq, Debug)]
enum Next {
    Bake,
    Publish,
    Wait,
}

fn next_step(baked: usize, interned: usize, published_is_newest: bool) -> Next {
    if baked < interned {
        Next::Bake
    } else if !published_is_newest {
        Next::Publish
    } else {
        Next::Wait
    }
}

fn start_baking(loader: &mut Loader, pool: &'static AsyncComputeTaskPool) {
    let Some(mut catalog) = loader.catalog.take() else {
        return;
    };
    let states = loader.palette.states.clone();
    let biomes = loader.palette.biomes.clone();
    let known = loader.sprites;
    let world = loader.newest.clone();
    let tints = std::mem::take(&mut loader.to_tint);
    let task = pool.spawn(async move {
        blocks::extend(&mut catalog, &states, &biomes);
        let blocks = catalog.blocks.clone();
        let sprites = (catalog.sprites.len() != known).then(|| {
            let sprites = &catalog.sprites;
            Upload::Sprites {
                atlases: sprites
                    .arrays()
                    .iter()
                    .map(|array| Atlas {
                        size: array.size,
                        layers: array.layers(),
                        mips: array.mip_chain(),
                    })
                    .collect(),
                animations: sprites
                    .animations()
                    .iter()
                    .map(|animation| Animation {
                        base_layer: sprites.base_layer(animation),
                        count: animation.count,
                        frametime: animation.frametime,
                        interpolate: u32::from(animation.interpolate),
                    })
                    .collect(),
                animated_from: sprites.animated_from(),
            }
        });
        Baked {
            catalog,
            blocks,
            sprites,
        }
    });
    loader.baking = Some((world, tints, task));
}

fn publish(
    loader: &mut Loader,
    world: Arc<World>,
    tinted: Vec<[i32; 2]>,
    baked: Option<Baked>,
    pool: &'static AsyncComputeTaskPool,
) {
    if let Some(baked) = baked {
        if let Some(sprites) = baked.sprites {
            loader.sprites = baked.catalog.sprites.len();
            loader.uploads.push(sprites);
        }
        loader.catalog = Some(baked.catalog);
        loader.blocks = Arc::new(baked.blocks);
    }
    let tints = loader
        .catalog
        .as_ref()
        .expect("the catalog is only away while baking")
        .tints
        .clone();
    assert!(
        world.states_reach() <= loader.blocks.len(),
        "a world reaching {} block states would be meshed against a table of {}",
        world.states_reach(),
        loader.blocks.len(),
    );
    loader.world = world;

    for coords in tinted {
        let corner = [
            (coords[0] - loader.world.min_region[0]) as usize * REGION_BLOCKS,
            (coords[1] - loader.world.min_region[1]) as usize * REGION_BLOCKS,
        ];
        let world = loader.world.clone();
        let tints = tints.clone();
        loader.tinting.push((
            [corner[0] as u32, corner[1] as u32],
            pool.spawn(async move { blocks::tint_square(&world, &tints, corner) }),
        ));
    }
}

fn report(loader: &Loader) {
    let catalog = loader.catalog.as_ref().expect("baking is finished");
    let took = loader.started.elapsed();
    println!(
        "{} region files loaded in {took:.2?}, {} sections, {} block states ({} unrenderable), \
         {} sprites",
        loader.world.loaded(),
        loader.world.non_empty_sections(),
        loader.palette.states.len(),
        catalog.failures.len(),
        catalog.sprites.len(),
    );
    for failure in &catalog.failures {
        println!("  skipping {failure}");
    }
    for (index, array) in catalog.sprites.arrays().iter().enumerate() {
        println!(
            "  sprite array {index}: {:>4} sprites at {}x{}, {} of them animated, \
             {} resident layers",
            array.sprites(),
            array.size,
            array.size,
            array.animated(),
            array.layers(),
        );
    }
    for stream in 0..STREAMS {
        let live = loader.regions.iter().map(|region| &region.lists[stream]);
        let groups: usize = live.clone().map(Vec::len).sum();
        let quads: u64 = live
            .flat_map(|list| list.iter())
            .map(|(_, group)| u64::from(group.quad_count))
            .sum();
        println!(
            "  {:<22} {quads:>9} quads in {groups:>6} groups",
            STREAM_NAMES[stream],
        );
    }
    let arena = |arena: &Arena, unit: usize| {
        (
            arena.asked(),
            (arena.asked() * unit) as f64 / 1e6,
            (arena.held() * unit) as f64 / 1e6,
            (arena.capacity() * unit) as f64 / 1e6,
            100.0 * arena.held() as f64 / arena.capacity() as f64,
        )
    };
    for (name, (count, asked, held, capacity, share)) in [
        ("greedy quads", arena(&loader.quads, QUAD_WORDS * 4)),
        ("model quads", arena(&loader.models, 4 * 3 * 4)),
        ("block faces", arena(&loader.faces, 4)),
        ("groups", arena(&loader.groups, size_of::<Group>())),
    ] {
        println!(
            "  {count} {name} are {asked:.1} MB, held in {held:.1} MB of {capacity:.0} \
             ({share:.0}% of the arena, {:.0}% of it rounding)",
            100.0 * (held - asked) / held.max(f64::MIN_POSITIVE),
        );
    }
    if loader.dropped > 0 {
        println!(
            "  {} sections sit outside the {} the world declares and are not drawn",
            loader.dropped,
            crate::anvil::SECTIONS_Y,
        );
    }
    if loader.evicted > 0 {
        println!(
            "  {} sections gave their room back to nearer ones",
            loader.evicted,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anvil::{REGION_CHUNKS, SECTIONS_Y};

    #[test]
    fn two_sections_at_a_threshold_cannot_take_each_others_room() {
        let (near, far) = (100.0, 100.0 + HYSTERESIS + 1.0);
        assert!(
            worth_evicting(far, near),
            "the far one gives way to the near one"
        );
        assert!(!worth_evicting(near, far), "and never the other way round");

        for other in [100.0, 100.5, 100.0 + HYSTERESIS] {
            assert!(!worth_evicting(other, near));
            assert!(!worth_evicting(near, other));
        }
    }

    #[test]
    fn a_file_that_brings_no_new_block_states_still_reaches_the_world() {
        assert_eq!(next_step(400, 400, false), Next::Publish);
        assert_eq!(next_step(400, 600, false), Next::Bake);
        assert_eq!(next_step(400, 400, true), Next::Wait);
    }

    fn loader(grid: RegionGrid) -> Loader {
        let layout = Arc::new(Layout {
            grid,
            min_section: [0; 3],
            quad_capacity: 1 << 12,
            model_capacity: 1 << 12,
            face_capacity: 1 << 16,
            group_capacity: 1 << 12,
            cave_words: 0,
            tint_origin: [0; 2],
            tint_size: [1; 2],
        });
        Loader::new(
            layout,
            Uploads::default(),
            Window {
                min_region: [0; 2],
                regions: [1; 2],
                files: Vec::new(),
            },
        )
    }

    fn one_greedy_group(section: [usize; 3], quads: u32) -> SectionMesh {
        let mut spans = [Default::default(); STREAMS];
        spans[0] = crate::mesh::StreamSpan {
            group_count: 1,
            quad_count: quads,
        };
        SectionMesh {
            section,
            simple: vec![[0; QUAD_WORDS]; quads as usize],
            faces: vec![0; 4],
            complex: Vec::new(),
            groups: vec![Group {
                quad_base: 0,
                quad_count: quads,
                section: 0,
                quad_prefix: 0,
            }],
            spans,
            connectivity: CONNECT_ALL,
        }
    }

    #[test]
    fn a_region_draws_the_groups_of_every_section_placed_in_it() {
        let grid = RegionGrid { x: 1, y: 1, z: 1 };
        let mut loader = loader(grid);
        let mut cave = CaveCull::new(grid, [0; 3], grid.extent());

        let near = loader
            .place(one_greedy_group([0, 0, 0], 3), &mut cave)
            .unwrap_or_else(|_| panic!("the arena has room"));
        let far = loader
            .place(one_greedy_group([1, 0, 0], 5), &mut cave)
            .unwrap_or_else(|_| panic!("the arena has room"));
        assert_ne!(
            near.quads.0, far.quads.0,
            "two sections cannot share arena room"
        );

        let flushed = loader.flush(0, &cave).expect("the group arena has room");
        assert_eq!(flushed.replaces, Some(0));
        assert_eq!(flushed.draws.len(), 1, "one bucket of one region");
        assert_eq!(flushed.draws[0].group_count, 2);
        assert_eq!(flushed.draws[0].quad_count, 8);
        assert_eq!(
            flushed
                .groups
                .1
                .iter()
                .map(|g| g.quad_prefix)
                .collect::<Vec<_>>(),
            [0, 3],
            "the blend order of a region runs across the sections in it"
        );

        loader.evict(grid.slot(0, 0, 0), &mut cave);
        let flushed = loader.flush(0, &cave).expect("the group arena has room");
        assert_eq!(flushed.draws[0].group_count, 1);
        assert_eq!(flushed.draws[0].quad_count, 5);
        assert_eq!(
            flushed.groups.1[0].quad_prefix, 0,
            "what the evicted section held is given back, not left as a hole"
        );
    }

    #[test]
    fn a_render_region_on_a_file_corner_reads_the_diagonal_file_too() {
        let grid = RegionGrid::covering([REGION_CHUNKS * 2, SECTIONS_Y, REGION_CHUNKS * 2]);
        assert_eq!([grid.x, grid.z], [4, 4]);

        let mut files = files_read(grid, [-1, -1], 5);
        files.sort();
        assert_eq!(files, [[-1, -1], [-1, 0], [0, -1], [0, 0]]);

        let mut files = files_read(grid, [-1, -1], 0);
        files.sort();
        assert_eq!(files, [[-2, -2], [-2, -1], [-1, -2], [-1, -1]]);
    }
}
