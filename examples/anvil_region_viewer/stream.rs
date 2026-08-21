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
use crate::mesh::{self, Batch, Draw, Group, STREAM_NAMES, STREAMS, Scratch, StreamSpan};
use crate::pack::{
    QUAD_WORDS, RENDER_REGION_X, RENDER_REGION_Y, RENDER_REGION_Z, RegionGrid,
    SECTIONS_PER_RENDER_REGION,
};
use crate::render::{Animation, Atlas, Layout, Placement, Upload, Uploads};

const HYSTERESIS: f32 = (RENDER_REGION_X * SECTION_SIZE) as f32;

const IN_FLIGHT: usize = 4;

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
    meshing: Vec<(usize, Task<Batch>)>,
    camera: Vec3,
    anchor: Vec3,
    evicted: usize,
    quads: Arena,
    models: Arena,
    faces: Arena,
    groups: Arena,
    dropped: usize,
    sprites: usize,
    streams: [StreamSpan; STREAMS],
    started: Instant,
    reported: bool,
}

struct Resident {
    quads: Block,
    models: Block,
    faces: Block,
    groups: Block,
    connectivity: Vec<(u32, u64)>,
}

struct Baked {
    catalog: Catalog,
    blocks: Vec<BlockInfo>,
    sprites: Option<Upload>,
}

pub struct Status {
    pub files: usize,
    pub files_total: usize,
    pub regions: usize,
    pub regions_total: usize,
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
            resident: (0..regions).map(|_| None).collect(),
            deferred: vec![false; regions],
            meshing: Vec::new(),
            camera: Vec3::ZERO,
            anchor: Vec3::splat(f32::MAX),
            evicted: 0,
            quads: Arena::new(layout_quads),
            models: Arena::new(layout_models),
            faces: Arena::new(layout_faces),
            groups: Arena::new(layout_groups),
            dropped: 0,
            sprites: 0,
            streams: [StreamSpan::default(); STREAMS],
            started: Instant::now(),
            reported: false,
        }
    }

    pub fn status(&self) -> Status {
        Status {
            files: self.world.loaded(),
            files_total: self.expected.len(),
            regions: self.resident.iter().filter(|slot| slot.is_some()).count(),
            regions_total: self.layout.grid.len(),
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
            && self.wanted().is_none()
    }

    fn ready_to_mesh(&self, region: usize) -> bool {
        files_read(self.layout.grid, self.world.min_region, region)
            .iter()
            .all(|coords| !self.expected.contains(coords) || self.world.holds(*coords))
    }

    fn distance(&self, region: usize) -> f32 {
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

    fn wanted(&self) -> Option<usize> {
        (0..self.layout.grid.len())
            .filter(|region| {
                self.resident[*region].is_none()
                    && !self.deferred[*region]
                    && !self.meshing.iter().any(|(at, _)| at == region)
                    && self.ready_to_mesh(*region)
            })
            .min_by(|a, b| self.distance(*a).total_cmp(&self.distance(*b)))
    }

    fn victim(&self, candidate: f32) -> Option<usize> {
        let held = (0..self.layout.grid.len()).filter(|region| self.resident[*region].is_some());
        let farthest = held.max_by(|a, b| self.distance(*a).total_cmp(&self.distance(*b)))?;
        worth_evicting(self.distance(farthest), candidate).then_some(farthest)
    }

    fn evict(&mut self, region: usize, cave: &mut CaveCull) {
        let Some(held) = self.resident[region].take() else {
            return;
        };
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.faces.free(held.faces);
        self.groups.free(held.groups);
        self.uploads.push(Upload::Drop(region as u32));
        if let Some(base) = self.cave_base(cave, region) {
            cave.forget(base);
        }
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
            let camera = if axis == 0 { self.camera.x } else { self.camera.z };
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

        let mut bases = Vec::new();
        for region in 0..self.layout.grid.len() {
            let Some(held) = self.resident[region].as_ref() else {
                continue;
            };
            match self.cave_base(cave, region) {
                Some(base) => {
                    cave.set_region(base, &held.connectivity);
                    bases.push((region as u32, base as u32));
                }
                None => bases.push((region as u32, cave.always_visible())),
            }
        }
        self.uploads.rebase(bases);
    }

    fn place(&mut self, batch: Batch, cave: &mut CaveCull) -> Result<Placement, Batch> {
        let cave_base = match self.cave_base(cave, batch.region) {
            Some(base) => {
                cave.set_region(base, &batch.connectivity);
                base as u32
            }
            None => cave.always_visible(),
        };
        let Some(quads) = self.quads.alloc(batch.simple.len()) else {
            return Err(batch);
        };
        let Some(models) = self.models.alloc(batch.model_quads()) else {
            self.quads.free(quads);
            return Err(batch);
        };
        let Some(faces) = self.faces.alloc(batch.faces.len()) else {
            self.quads.free(quads);
            self.models.free(models);
            return Err(batch);
        };
        let Some(groups) = self.groups.alloc(batch.groups.len()) else {
            self.quads.free(quads);
            self.models.free(models);
            self.faces.free(faces);
            return Err(batch);
        };

        let mut placed = batch.groups;
        let mut draws = Vec::new();
        let mut first = 0usize;
        for stream in 0..STREAMS {
            let span = batch.spans[stream];
            if span.group_count == 0 {
                continue;
            }
            let base = if stream % 2 == 0 {
                quads.offset
            } else {
                models.offset
            } as u32;
            self.streams[stream].group_count += span.group_count;
            self.streams[stream].quad_count += span.quad_count;
            let run = span.group_count as usize;
            for group in &mut placed[first..first + run] {
                group.quad_base += base;
            }
            draws.push(Draw {
                stream: stream as u32,
                region: batch.region as u32,
                origin: self.layout.grid.origin(self.layout.min_section, batch.region),
                cave_base,
                face_base: faces.offset as u32,
                first_group: (groups.offset + first) as u32,
                group_count: span.group_count,
                quad_count: span.quad_count,
            });
            first += run;
        }

        self.resident[batch.region] = Some(Resident {
            quads,
            models,
            faces,
            groups,
            connectivity: batch.connectivity,
        });
        Ok(Placement {
            quads: ((quads.offset * QUAD_WORDS * 4) as u64, batch.simple),
            vertices: ((models.offset * 4 * 3 * 4) as u64, batch.complex),
            faces: ((faces.offset * 4) as u64, batch.faces),
            groups: ((groups.offset * size_of::<Group>()) as u64, placed),
            draws,
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
    loader.parsing.retain_mut(|(coords, task)| match check_ready(task) {
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
        let (world, tinted) = loader.baking.take().map(|(w, t, _)| (w, t)).expect("just held");
        publish(loader, world, tinted, Some(baked), pool);
    }
    settle(loader, pool);

    let mut tinted = Vec::new();
    loader.tinting.retain_mut(|(origin, task)| match check_ready(task) {
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
    loader.meshing.retain_mut(|(_, task)| match check_ready(task) {
        Some(batch) => {
            meshed.push(batch);
            false
        }
        None => true,
    });
    for batch in meshed {
        let region = batch.region;
        let here = loader.distance(region);
        let mut pending = batch;
        loop {
            match loader.place(pending, &mut cave) {
                Ok(placement) => {
                    if !placement.draws.is_empty() {
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
                        loader.deferred[region] = true;
                        break;
                    }
                },
            }
        }
    }

    while loader.parsing.len() < IN_FLIGHT
        && let Some((coords, path)) = loader.to_parse.pop()
    {
        loader
            .parsing
            .push((coords, pool.spawn(async move { anvil::load(&path) })));
    }

    while loader.meshing.len() < IN_FLIGHT
        && let Some(region) = loader.wanted()
    {
        let world = loader.world.clone();
        let blocks = loader.blocks.clone();
        let grid = loader.layout.grid;
        loader.meshing.push((
            region,
            pool.spawn(async move {
                let mut scratch = Scratch::new();
                mesh::mesh_render_region(&world, &blocks, grid, region, &mut scratch)
            }),
        ));
    }

    if loader.idle() && !loader.reported {
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
    for (stream, span) in loader.streams.iter().enumerate() {
        println!(
            "  {:<22} {:>9} quads in {:>6} groups",
            STREAM_NAMES[stream], span.quad_count, span.group_count,
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
            "  {} render regions gave their room back to nearer ones",
            loader.evicted,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anvil::{REGION_CHUNKS, SECTIONS_Y};

    #[test]
    fn two_regions_at_a_threshold_cannot_take_each_others_room() {
        let (near, far) = (100.0, 100.0 + HYSTERESIS + 1.0);
        assert!(worth_evicting(far, near), "the far one gives way to the near one");
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
