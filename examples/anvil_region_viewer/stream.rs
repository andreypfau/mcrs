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
use crate::blocks::{self, BlockInfo, Catalog};
use crate::cave::CaveCull;
use crate::mesh::{self, Batch, Draw, Group, STREAM_NAMES, STREAMS, Scratch, StreamSpan};
use crate::pack::{QUAD_WORDS, RENDER_REGION_X, RENDER_REGION_Z, RegionGrid};
use crate::render::{Animation, Atlas, Layout, Placement, Upload, Uploads};

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
    to_mesh: Vec<usize>,
    meshing: Vec<Task<Batch>>,
    /// Where the next batch lands in each arena, in that arena's own units.
    quad_next: usize,
    model_next: usize,
    group_next: usize,
    /// Sections of a file that fall outside the world's declared height.
    dropped: usize,
    /// Render regions whose geometry did not fit the arenas.
    overflowed: usize,
    /// Sprites the render world has already been told about.
    sprites: usize,
    /// What each stream holds, for the report.
    streams: [StreamSpan; STREAMS],
    started: Instant,
    reported: bool,
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
    pub quads: f32,
    pub models: f32,
    pub overflowed: usize,
}

impl Loader {
    pub fn new(layout: Arc<Layout>, uploads: Uploads, window: Window) -> Self {
        let world = World::new(window.min_region, window.regions);
        let mut to_mesh: Vec<usize> = (0..layout.grid.len()).collect();
        // Nearest first, so what the camera starts inside of turns up before the far corners.
        let middle = [
            layout.grid.x as f32 / 2.0 - 0.5,
            layout.grid.z as f32 / 2.0 - 0.5,
        ];
        to_mesh.sort_by(|a, b| {
            let key = |region: &usize| {
                let [x, _, z] = layout.grid.corner(*region);
                let (dx, dz) = (
                    x as f32 / RENDER_REGION_X as f32 - middle[0],
                    z as f32 / RENDER_REGION_Z as f32 - middle[1],
                );
                dx * dx + dz * dz
            };
            key(b).total_cmp(&key(a))
        });
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
            to_mesh,
            meshing: Vec::new(),
            quad_next: 0,
            model_next: 0,
            group_next: 0,
            dropped: 0,
            overflowed: 0,
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
            regions: self.layout.grid.len() - self.to_mesh.len() - self.meshing.len(),
            regions_total: self.layout.grid.len(),
            quads: self.quad_next as f32 / self.layout.quad_capacity as f32,
            models: self.model_next as f32 / self.layout.model_capacity as f32,
            overflowed: self.overflowed,
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
            && self.to_mesh.is_empty()
            && self.meshing.is_empty()
    }

    /// Whether a render region can be meshed: every file it reads from is either in or known
    /// never to arrive.
    fn ready_to_mesh(&self, region: usize) -> bool {
        files_read(self.layout.grid, self.world.min_region, region)
            .iter()
            .all(|coords| !self.expected.contains(coords) || self.world.holds(*coords))
    }

    /// Gives a batch its place in the arenas and turns it into the draws that will name it.
    ///
    /// The offsets are handed out here rather than in the render world because this is also where
    /// the decision to make room for something will live.
    fn place(&mut self, batch: Batch) -> Option<Placement> {
        let quads = batch.simple.len();
        let models = batch.model_quads();
        let groups = batch.groups.len();
        if self.quad_next + quads > self.layout.quad_capacity
            || self.model_next + models > self.layout.model_capacity
            || self.group_next + groups > self.layout.group_capacity
        {
            self.overflowed += 1;
            return None;
        }

        let mut placed = batch.groups;
        let mut draws = Vec::new();
        let mut first = 0usize;
        for stream in 0..STREAMS {
            let span = batch.spans[stream];
            if span.group_count == 0 {
                continue;
            }
            // A group's quad_base is an index into the arena its stream draws from, and the two
            // arenas fill at their own rates.
            let base = if stream % 2 == 0 {
                self.quad_next
            } else {
                self.model_next
            } as u32;
            self.streams[stream].group_count += span.group_count;
            self.streams[stream].quad_count += span.quad_count;
            let run = span.group_count as usize;
            for group in &mut placed[first..first + run] {
                group.quad_base += base;
            }
            draws.push(Draw {
                stream: stream as u32,
                origin: self.layout.grid.origin(self.layout.min_section, batch.region),
                cave_base: self.layout.grid.cave_base(batch.region) as u32,
                first_group: (self.group_next + first) as u32,
                group_count: span.group_count,
                quad_count: span.quad_count,
            });
            first += run;
        }

        let placement = Placement {
            quads: ((self.quad_next * QUAD_WORDS * 4) as u64, batch.simple),
            vertices: ((self.model_next * 4 * 3 * 4) as u64, batch.complex),
            groups: ((self.group_next * size_of::<Group>()) as u64, placed),
            draws,
        };
        self.quad_next += quads;
        self.model_next += models;
        self.group_next += groups;
        Some(placement)
    }
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
pub fn advance(mut loader: ResMut<Loader>, mut cave: ResMut<CaveCull>) {
    let pool = AsyncComputeTaskPool::get();
    let loader = &mut *loader;

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
    loader.meshing.retain_mut(|task| match check_ready(task) {
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
        if batch.is_empty() {
            continue;
        }
        let region = batch.region;
        match loader.place(batch) {
            Some(placement) => loader.uploads.push(Upload::Geometry(placement)),
            None => println!(
                "render region {region} does not fit the arena and is missing from the frame; \
                 raise ANVIL_ARENA",
            ),
        }
    }

    while loader.parsing.len() < IN_FLIGHT
        && let Some((coords, path)) = loader.to_parse.pop()
    {
        loader
            .parsing
            .push((coords, pool.spawn(async move { anvil::load(&path) })));
    }

    while loader.meshing.len() < IN_FLIGHT {
        let Some(at) = loader
            .to_mesh
            .iter()
            .rposition(|region| loader.ready_to_mesh(*region))
        else {
            break;
        };
        let region = loader.to_mesh.remove(at);
        let world = loader.world.clone();
        let blocks = loader.blocks.clone();
        let grid = loader.layout.grid;
        loader.meshing.push(pool.spawn(async move {
            let mut scratch = Scratch::new();
            mesh::mesh_render_region(&world, &blocks, grid, region, &mut scratch)
        }));
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
    let quad_bytes = loader.quad_next * QUAD_WORDS * 4;
    let model_bytes = loader.model_next * 4 * 3 * 4;
    println!(
        "  {} greedy quads in {:.1} MB of {:.0} ({:.0}% of the arena), \
         {} model quads in {:.1} MB of {:.0} ({:.0}%)",
        loader.quad_next,
        quad_bytes as f64 / 1e6,
        (loader.layout.quad_capacity * QUAD_WORDS * 4) as f64 / 1e6,
        100.0 * loader.quad_next as f64 / loader.layout.quad_capacity as f64,
        loader.model_next,
        model_bytes as f64 / 1e6,
        (loader.layout.model_capacity * 4 * 3 * 4) as f64 / 1e6,
        100.0 * loader.model_next as f64 / loader.layout.model_capacity as f64,
    );
    if loader.dropped > 0 {
        println!(
            "  {} sections sit outside the {} the world declares and are not drawn",
            loader.dropped,
            crate::anvil::SECTIONS_Y,
        );
    }
    if loader.overflowed > 0 {
        println!(
            "  {} render regions did not fit the arena and are missing from the frame",
            loader.overflowed,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anvil::{REGION_CHUNKS, SECTIONS_Y};

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
