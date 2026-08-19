//! Loading the window in the background.
//!
//! Two stages, with different conditions for being ready. Parsing a region file needs nothing but
//! the file. Meshing a render region needs its own file *and* every file the mesher's one-block
//! border reaches into, because a section meshed against a neighbour that has not arrived reads
//! air there and comes out with a wall of faces down the seam, lit as if it faced open sky.
//!
//! Nothing here locks. A world is a rectangle of handles to parsed files, so the snapshot a mesher
//! is handed costs a few pointer copies and stays fixed under it while later files keep arriving
//! into a world that shares all the same ones.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};

use crate::anvil::{self, Palette, REGION_BLOCKS, Region, SECTION_SIZE, Window, World};
use crate::arena::{Arena, Block};
use crate::blocks::{self, BlockInfo, Catalog};
use crate::cave::CaveCull;
use crate::mesh::{self, Batch, Draw, Group, STREAM_NAMES, STREAMS, Scratch, StreamSpan};
use crate::pack::{
    QUAD_WORDS, RENDER_REGION_X, RENDER_REGION_Y, RENDER_REGION_Z, RegionGrid,
    SECTIONS_PER_RENDER_REGION,
};
use crate::render::{Animation, Atlas, Layout, Placement, Upload, Uploads};

/// How much farther a region has to be than the one asking for its room before it is given up, in
/// blocks. One render region's width.
///
/// Without it a region on the very edge of what fits is dropped and re-meshed every time the
/// camera moves a hair, which costs a tenth of a second of meshing each way and shows as the arena
/// occupancy sawing while nobody is touching the mouse.
const HYSTERESIS: f32 = (RENDER_REGION_X * SECTION_SIZE) as f32;

/// Tasks of one kind running at once. Each parse holds a whole expanded region file and each mesh
/// holds the geometry it has produced so far, so this is a memory bound rather than a throughput
/// one: the pool has more threads than this and spends them on the other kind.
const IN_FLIGHT: usize = 4;

/// Everything still to be loaded, and everything loaded so far.
#[derive(Resource)]
pub struct Loader {
    layout: Arc<Layout>,
    uploads: Uploads,
    palette: Palette,
    /// Away while a baking task holds it.
    catalog: Option<Catalog>,
    /// What a mesher reads. Replaced, not mutated, once the states a file brought are baked.
    world: Arc<World>,
    /// Every file interned so far, published or not. A file that lands while an earlier one is
    /// still baking joins this and rides the same publish.
    newest: Arc<World>,
    /// The only part of the catalog a mesher needs. Kept apart so a mesher never holds the sprite
    /// pixels or the biome colours alive.
    blocks: Arc<Vec<BlockInfo>>,
    /// Region files that exist, so a file that is simply not there is never waited on.
    expected: HashSet<[i32; 2]>,
    to_parse: Vec<([i32; 2], PathBuf)>,
    parsing: Vec<([i32; 2], Task<Result<Region, String>>)>,
    tinting: Vec<([u32; 2], Task<Vec<u8>>)>,
    /// Baking touches the filesystem and parses JSON, so it is the one main-thread cost worth
    /// tens of milliseconds. One at a time: it owns the catalog while it runs.
    ///
    /// The world and the tint list are the ones the baking states were taken from. Publishing
    /// anything newer would hand a mesher a world whose block ids the baked catalog does not
    /// reach, which is an index straight past the end of it on a pool thread.
    baking: Option<(Arc<World>, Vec<[i32; 2]>, Task<Baked>)>,
    /// Files whose biome colours still have to be drawn, once their biomes are baked.
    to_tint: Vec<[i32; 2]>,
    /// What each render region holds, so it can be given back.
    resident: Vec<Option<Resident>>,
    /// Regions that were meshed and found no room. Retried when room comes free or the camera has
    /// moved far enough for the order to have really changed, rather than every frame: meshing one
    /// costs a tenth of a second whether or not it fits.
    deferred: Vec<bool>,
    meshing: Vec<(usize, Task<Batch>)>,
    /// Where the camera is, and where it stood when the deferred list was last cleared.
    camera: Vec3,
    anchor: Vec3,
    evicted: usize,
    /// The three arenas, in their own units: greedy quads, model quads, culling groups.
    quads: Arena,
    models: Arena,
    groups: Arena,
    /// Sections of a file that fall outside the world's declared height.
    dropped: usize,
    /// Sprites the render world has already been told about.
    sprites: usize,
    /// What each stream holds, for the report.
    streams: [StreamSpan; STREAMS],
    started: Instant,
    reported: bool,
}

/// The room one render region's geometry holds in the arenas.
struct Resident {
    quads: Block,
    models: Block,
    groups: Block,
}

/// What one round of baking produced.
struct Baked {
    catalog: Catalog,
    /// The only part of it a mesher needs.
    blocks: Vec<BlockInfo>,
    /// Present when the sprite set grew, which is what makes the atlas need rebuilding.
    sprites: Option<Upload>,
}

/// What the counter line shows about loading.
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
        let (layout_quads, layout_models, layout_groups) = (
            layout.quad_capacity,
            layout.model_capacity,
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

    /// Whether everything the window covers is loaded and on the GPU. Geometry reaches the GPU a
    /// slice at a time, so the loader going quiet is not on its own enough.
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

    /// Whether a render region can be meshed: every file it reads from is either in or known
    /// never to arrive.
    fn ready_to_mesh(&self, region: usize) -> bool {
        files_read(self.layout.grid, self.world.min_region, region)
            .iter()
            .all(|coords| !self.expected.contains(coords) || self.world.holds(*coords))
    }

    /// How far the camera is from a render region's box, in blocks. Zero inside it.
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

    /// The nearest render region worth meshing next, if any: not already held, not waiting on a
    /// file, not already being meshed, and not one that has just been found too large to fit.
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

    /// The region to make room with, or `None` when nothing held is far enough behind the one
    /// asking to be worth the swap.
    fn victim(&self, candidate: f32) -> Option<usize> {
        let held = (0..self.layout.grid.len()).filter(|region| self.resident[*region].is_some());
        let farthest = held.max_by(|a, b| self.distance(*a).total_cmp(&self.distance(*b)))?;
        worth_evicting(self.distance(farthest), candidate).then_some(farthest)
    }

    /// Takes a region's room back. Its draws leave the table before anything is written over the
    /// blocks it held, which the upload queue being drained in order is what guarantees.
    fn evict(&mut self, region: usize, cave: &mut CaveCull) {
        let Some(held) = self.resident[region].take() else {
            return;
        };
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.groups.free(held.groups);
        self.uploads.push(Upload::Drop(region as u32));
        cave.forget(
            self.layout.grid.cave_base(region),
            SECTIONS_PER_RENDER_REGION,
        );
        self.evicted += 1;
        // Room has come free, so what did not fit before may fit now.
        self.deferred.fill(false);
    }

    /// Gives a batch its place in the arenas and turns it into the draws that will name it.
    ///
    /// The offsets are handed out here rather than in the render world because this is also where
    /// the decision to make room for something will live.
    fn place(&mut self, batch: Batch) -> Result<Placement, Batch> {
        // All three or none: a region half in the arena would leave groups pointing at quads that
        // were never written.
        let Some(quads) = self.quads.alloc(batch.simple.len()) else {
            return Err(batch);
        };
        let Some(models) = self.models.alloc(batch.model_quads()) else {
            self.quads.free(quads);
            return Err(batch);
        };
        let Some(groups) = self.groups.alloc(batch.groups.len()) else {
            self.quads.free(quads);
            self.models.free(models);
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
            // A group's quad_base is an index into the arena its stream draws from, and the two
            // arenas hand out their blocks independently.
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
                cave_base: self.layout.grid.cave_base(batch.region) as u32,
                first_group: (groups.offset + first) as u32,
                group_count: span.group_count,
                quad_count: span.quad_count,
            });
            first += run;
        }

        self.resident[batch.region] = Some(Resident {
            quads,
            models,
            groups,
        });
        Ok(Placement {
            quads: ((quads.offset * QUAD_WORDS * 4) as u64, batch.simple),
            vertices: ((models.offset * 4 * 3 * 4) as u64, batch.complex),
            groups: ((groups.offset * size_of::<Group>()) as u64, placed),
            draws,
        })
    }
}

/// Whether a region that far away should give its room to one this near.
///
/// The margin is what stops the two from trading places: for a swap to reverse, each would have to
/// be the clearly farther one, which cannot hold both ways at once.
fn worth_evicting(resident: f32, candidate: f32) -> bool {
    resident > candidate + HYSTERESIS
}

/// The region files a render region's mesher reads from.
///
/// The mesher fills a one-block border around every section it touches, and ambient occlusion
/// reaches diagonally past a corner on top of that, so the box read runs one block past the render
/// region on each side. A region file is thirty-two sections across and a render region sixteen,
/// so every render region is a corner quadrant of its file and that box always lands in a two by
/// two block of files — the file itself, the two across its edges, and the one across its corner.
/// Waiting only on the four edge neighbours leaves a one-block column of wrong ambient occlusion
/// running the height of the world at every file corner, which reads as terrain rather than as a
/// fault.
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

/// Starts what can be started, collects what has finished, and hands the results on.
///
/// A finished task is observed and dropped in the same pass: polling one again after it has given
/// up its value panics, and dropping one that has not finished cancels it.
pub fn advance(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let pool = AsyncComputeTaskPool::get();
    let loader = &mut *loader;
    loader.camera = camera.translation();
    // A region found too large to fit is only reconsidered once the view has really moved, not
    // every frame: meshing one costs a tenth of a second whether or not it turns out to fit.
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
                // A file that will not parse is a fact about the world, not a bug to bring the
                // viewer down over. It has to stop being waited on, though, or every render region
                // whose border reaches into it waits for it forever and stays empty in silence.
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
        // Applied whether or not the geometry found room: the world really is solid there, and a
        // walk that believed otherwise would light up everything behind it.
        cave.set_connectivity(&batch.connectivity);
        let region = batch.region;
        let here = loader.distance(region);
        let mut pending = batch;
        loop {
            match loader.place(pending) {
                Ok(placement) => {
                    // A region of open sky holds nothing and still counts as loaded, or it would
                    // be meshed again every frame for as long as the view covered it.
                    if !placement.draws.is_empty() {
                        loader.uploads.push(Upload::Geometry(placement));
                    }
                    break;
                }
                // Memory is the rule and distance only the order: room comes only from a region
                // clearly farther away, and otherwise this one waits rather than starting a swap
                // that would undo itself next frame.
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

/// Takes a parsed file's ids into the shared tables. The world it joins is not published until
/// its block states are baked, because a mesher handed a world whose states the catalog does not
/// yet cover would index straight past the end of it.
fn absorb(loader: &mut Loader, coords: [i32; 2], region: Region) {
    let mut world = (*loader.newest).clone();
    loader.dropped += world.insert(&mut loader.palette, coords, region);
    loader.newest = Arc::new(world);
    loader.to_tint.push(coords);
}

/// Moves the world on as far as it can: bakes what the palette has gained, or, when it has gained
/// nothing, simply publishes the files that have joined.
///
/// The second half is not an optimisation. A file whose blocks the world has already seen brings
/// no new states at all, and hanging publication on there being something to bake would leave that
/// file parsed, resident, and never drawn.
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

/// What the loader can do next with what it has parsed.
#[derive(PartialEq, Eq, Debug)]
enum Next {
    /// Block states have been interned that the catalog has not baked.
    Bake,
    /// Nothing left to bake, but files have joined the world since it was last published.
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

/// Bakes whatever the palette has gained, off the main thread.
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

/// Publishes what baking produced: the world and the block table together, then the atlas, then
/// the biome colours of every file that has been waiting for them.
///
/// Order matters on the upload queue. A quad's sprite number only means anything beside the atlas
/// it indexes, so the atlas has to be down before any geometry that names it, and the queue is
/// drained in the order it is filled.
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

    /// The rule that stops two regions trading places. For a swap to reverse, each would have to
    /// be the clearly farther one, and that cannot hold both ways at once — so a camera sitting
    /// still on a threshold cannot make the arena occupancy saw.
    #[test]
    fn two_regions_at_a_threshold_cannot_take_each_others_room() {
        let (near, far) = (100.0, 100.0 + HYSTERESIS + 1.0);
        assert!(worth_evicting(far, near), "the far one gives way to the near one");
        assert!(!worth_evicting(near, far), "and never the other way round");

        // Anything inside the margin is a stand-off, whichever way round it is asked.
        for other in [100.0, 100.5, 100.0 + HYSTERESIS] {
            assert!(!worth_evicting(other, near));
            assert!(!worth_evicting(near, other));
        }
    }

    /// A file whose blocks the world has already seen brings no new states at all. Hanging
    /// publication on there being something to bake leaves that file parsed, resident, and never
    /// drawn — with the counter reading zero files loaded and nothing else to say why.
    #[test]
    fn a_file_that_brings_no_new_block_states_still_reaches_the_world() {
        assert_eq!(next_step(400, 400, false), Next::Publish);
        assert_eq!(next_step(400, 600, false), Next::Bake);
        assert_eq!(next_step(400, 400, true), Next::Wait);
    }

    /// A render region on the inside corner of a file reads across both of its edges and across
    /// the corner between them. Missing the diagonal one leaves a column of wrong ambient
    /// occlusion running the whole height of the world where four files meet.
    #[test]
    fn a_render_region_on_a_file_corner_reads_the_diagonal_file_too() {
        let grid = RegionGrid::covering([REGION_CHUNKS * 2, SECTIONS_Y, REGION_CHUNKS * 2]);
        assert_eq!([grid.x, grid.z], [4, 4]);

        // The quadrant of r.-1.-1 that touches r.0.0 at a point.
        let mut files = files_read(grid, [-1, -1], 5);
        files.sort();
        assert_eq!(files, [[-1, -1], [-1, 0], [0, -1], [0, 0]]);

        // The quadrant at the window's own outer corner reads outside it, where nothing will ever
        // arrive and air is the right answer.
        let mut files = files_read(grid, [-1, -1], 0);
        files.sort();
        assert_eq!(files, [[-2, -2], [-2, -1], [-1, -2], [-1, -1]]);
    }
}
