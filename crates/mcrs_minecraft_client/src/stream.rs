use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures::check_ready};

use crate::anvil::{self, Palette, REGION_BLOCKS, Region, SECTION_SIZE, Window, World};
use crate::arena::{Arena, Block};
use crate::blocks::{self, BlockInfo, Catalog};
use crate::cave::{CaveCull, NO_SLOT};
use crate::mesh::{self, Draw, Group, STREAM_NAMES, STREAMS, Scratch, SectionMesh};
use crate::model::Pack;
use crate::pack::QUAD_WORDS;
use crate::render::{Animation, Atlas, Budget, Placement, SectionDesc, Upload, Uploads};

const HYSTERESIS: f32 = (16 * SECTION_SIZE) as f32;

const FILES_IN_FLIGHT: usize = 4;

const SECTIONS_IN_FLIGHT: usize = 128;

const SECTIONS_PER_FRAME: usize = 32;

#[derive(Resource)]
pub struct Loader {
    uploads: Uploads,
    palette: Palette,
    pack: PackLoad,
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
    extent: [usize; 3],
    min_section: [i32; 3],
    resident: Vec<Option<Resident>>,
    deferred: Vec<bool>,
    pending: Vec<bool>,
    meshing: Vec<(usize, u32, Task<SectionMesh>)>,
    lists: [Vec<Group>; STREAMS],
    group_block: Block,
    slots: usize,
    free_slots: Vec<u32>,
    dead: Vec<u32>,
    slots_used: u32,
    live: usize,
    dirty: bool,
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

enum PackLoad {
    Pending,
    Loading(Task<Result<Pack, String>>),
    Ready(Arc<Pack>),
}

struct Resident {
    quads: Block,
    models: Block,
    faces: Block,
    slot: u32,
    connectivity: u64,
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
    pub fn new(budget: &Budget, uploads: Uploads, window: Window) -> Self {
        let world = World::new(window.min_region, window.regions);
        let cells = world.sections[0] * world.sections[1] * world.sections[2];
        Self {
            expected: window.files.iter().map(|(coords, _)| *coords).collect(),
            to_parse: window.files,
            uploads,
            palette: Palette::new(),
            pack: PackLoad::Pending,
            catalog: Some(blocks::empty()),
            extent: world.sections,
            min_section: world.min_section,
            world: Arc::new(world.clone()),
            newest: Arc::new(world),
            blocks: Arc::new(Vec::new()),
            parsing: Vec::new(),
            baking: None,
            to_tint: Vec::new(),
            tinting: Vec::new(),
            resident: (0..cells).map(|_| None).collect(),
            deferred: vec![false; cells],
            pending: vec![false; cells],
            meshing: Vec::new(),
            lists: std::array::from_fn(|_| Vec::new()),
            group_block: Block::EMPTY,
            slots: budget.sections,
            free_slots: Vec::new(),
            dead: Vec::new(),
            slots_used: 0,
            live: 0,
            dirty: false,
            camera: Vec3::ZERO,
            anchor: Vec3::splat(f32::MAX),
            evicted: 0,
            quads: Arena::new(budget.quads),
            models: Arena::new(budget.models),
            faces: Arena::new(budget.faces),
            groups: Arena::new(budget.groups),
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
            sections: self.live,
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
            && !self.dirty
            && self.wanted(1).is_empty()
    }

    fn cell(&self, [sx, sy, sz]: [usize; 3]) -> usize {
        (sy * self.extent[2] + sz) * self.extent[0] + sx
    }

    fn section_at(&self, cell: usize) -> [usize; 3] {
        let rest = cell / self.extent[0];
        [
            cell % self.extent[0],
            rest / self.extent[2],
            rest % self.extent[2],
        ]
    }

    fn absolute(&self, section: [usize; 3]) -> [i32; 3] {
        std::array::from_fn(|axis| section[axis] as i32 + self.min_section[axis])
    }

    fn ready_to_mesh(&self, sx: usize, sz: usize) -> bool {
        files_read(self.world.min_region, sx, sz)
            .iter()
            .all(|coords| !self.expected.contains(coords) || self.world.holds(*coords))
    }

    fn distance(&self, cell: usize) -> f32 {
        let section = self.absolute(self.section_at(cell));
        let min = Vec3::from_array(section.map(|n| (n * SECTION_SIZE as i32) as f32));
        let max = min + Vec3::splat(SECTION_SIZE as f32);
        (self.camera.clamp(min, max) - self.camera).length()
    }

    /// The nearest sections that hold blocks, have nowhere to be but the arena, and border only
    /// files that have arrived. Nothing more than distance decides the order yet.
    fn wanted(&self, want: usize) -> Vec<usize> {
        if want == 0 {
            return Vec::new();
        }
        let [ex, ey, ez] = self.extent;
        let ready: Vec<bool> = (0..ex * ez)
            .map(|at| self.ready_to_mesh(at % ex, at / ex))
            .collect();
        let mut nearest: Vec<(f32, usize)> = Vec::new();
        for sy in 0..ey {
            for sz in 0..ez {
                for sx in 0..ex {
                    if !ready[sz * ex + sx] || self.world.section(sx, sy, sz).is_none() {
                        continue;
                    }
                    let cell = self.cell([sx, sy, sz]);
                    if self.resident[cell].is_some() || self.deferred[cell] || self.pending[cell] {
                        continue;
                    }
                    nearest.push((self.distance(cell), cell));
                }
            }
        }
        if nearest.len() > want {
            nearest.select_nth_unstable_by(want, |a, b| a.0.total_cmp(&b.0));
            nearest.truncate(want);
        }
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        nearest.into_iter().map(|(_, cell)| cell).collect()
    }

    fn victim(&self, candidate: f32) -> Option<usize> {
        let farthest = (0..self.resident.len())
            .filter(|cell| self.resident[*cell].is_some())
            .max_by(|a, b| self.distance(*a).total_cmp(&self.distance(*b)))?;
        worth_evicting(self.distance(farthest), candidate).then_some(farthest)
    }

    fn take_slot(&mut self) -> Option<u32> {
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }
        ((self.slots_used as usize) < self.slots).then(|| {
            self.slots_used += 1;
            self.slots_used - 1
        })
    }

    fn evict(&mut self, cell: usize, cave: &mut CaveCull) {
        let Some(held) = self.resident[cell].take() else {
            return;
        };
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.faces.free(held.faces);
        if held.slot != NO_SLOT {
            self.dead.push(held.slot);
            self.dirty = true;
        }
        cave.forget(self.absolute(self.section_at(cell)));
        self.live -= 1;
        self.evicted += 1;
        self.deferred.fill(false);
    }

    /// A slot goes back on the free list only once the group records naming it are gone, so a
    /// section placed later in the same frame cannot have its groups swept away with them.
    fn sweep(&mut self) {
        if self.dead.is_empty() {
            return;
        }
        let dead = std::mem::take(&mut self.dead);
        for list in &mut self.lists {
            list.retain(|group| !dead.contains(&group.section));
        }
        self.free_slots.extend(dead);
    }

    fn follow(&mut self, cave: &mut CaveCull) {
        if !cave.follow(self.camera) {
            return;
        }
        for cell in 0..self.resident.len() {
            let Some((slot, mask)) = self.resident[cell]
                .as_ref()
                .map(|held| (held.slot, held.connectivity))
            else {
                continue;
            };
            cave.set_section(self.absolute(self.section_at(cell)), slot, mask);
        }
    }

    fn reserve(&mut self, mesh: &SectionMesh) -> Option<[Block; 3]> {
        let quads = self.quads.alloc(mesh.simple.len());
        let models = self.models.alloc(mesh.model_quads());
        let faces = self.faces.alloc(mesh.faces.len());
        match (quads, models, faces) {
            (Some(quads), Some(models), Some(faces)) => Some([quads, models, faces]),
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
                None
            }
        }
    }

    fn place(
        &mut self,
        mesh: SectionMesh,
        slot: u32,
        cave: &mut CaveCull,
    ) -> Result<Placement, SectionMesh> {
        let Some([quads, models, faces]) = self.reserve(&mesh) else {
            return Err(mesh);
        };

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
            self.lists[stream].extend_from_slice(&placed[first..first + run]);
            first += run;
        }

        let section = self.absolute(mesh.section);
        let cell = self.cell(mesh.section);
        let slot = if placed.is_empty() {
            self.free_slots.push(slot);
            NO_SLOT
        } else {
            self.dirty = true;
            slot
        };
        cave.set_section(section, slot, mesh.connectivity);
        self.resident[cell] = Some(Resident {
            quads,
            models,
            faces,
            slot,
            connectivity: mesh.connectivity,
        });
        self.live += 1;

        Ok(Placement {
            quads: ((quads.offset * QUAD_WORDS * 4) as u64, mesh.simple),
            vertices: ((models.offset * 4 * 3 * 4) as u64, mesh.complex),
            faces: ((faces.offset * 4) as u64, mesh.faces),
            sections: match slot {
                NO_SLOT => (0, Vec::new()),
                slot => (
                    (slot as usize * size_of::<SectionDesc>()) as u64,
                    vec![SectionDesc {
                        section,
                        scale: 1,
                        face_base: faces.offset as u32,
                    }],
                ),
            },
            ..Placement::default()
        })
    }

    /// The whole draw list hands its group block back and takes a fresh one, so the block the
    /// buckets are being drawn from is never the block being written.
    fn flush(&mut self) -> Option<Placement> {
        let total: usize = self.lists.iter().map(Vec::len).sum();
        let block = self.groups.alloc(total)?;
        let stale = std::mem::replace(&mut self.group_block, block);
        self.groups.free(stale);

        let mut records = Vec::with_capacity(total);
        let mut draws = Vec::with_capacity(STREAMS);
        for stream in 0..STREAMS {
            let first = records.len();
            let mut quads = 0u32;
            for group in &self.lists[stream] {
                let mut group = *group;
                group.quad_prefix = quads;
                quads += group.quad_count;
                records.push(group);
            }
            draws.push(Draw {
                stream: stream as u32,
                first_group: (block.offset + first) as u32,
                group_count: (records.len() - first) as u32,
                quad_count: quads,
            });
        }
        Some(Placement {
            groups: ((block.offset * size_of::<Group>()) as u64, records),
            draws: Some(draws),
            ..Placement::default()
        })
    }
}

fn worth_evicting(resident: f32, candidate: f32) -> bool {
    resident > candidate + HYSTERESIS
}

/// A section reads one block past its own faces, so it borders up to four region files.
fn files_read(min_region: [i32; 2], sx: usize, sz: usize) -> [[i32; 2]; 4] {
    let size = SECTION_SIZE as i32;
    let span = REGION_BLOCKS as i32;
    let xs = [sx as i32 * size - 1, (sx + 1) as i32 * size];
    let zs = [sz as i32 * size - 1, (sz + 1) as i32 * size];
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
    assets: Res<AssetServer>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let pool = AsyncComputeTaskPool::get();
    let loader = &mut *loader;
    let pack = poll_pack(loader, &assets);
    loader.camera = camera.translation();
    loader.follow(&mut cave);
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
    if let Some(pack) = pack {
        settle(loader, &pack, pool);
    }

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
        .retain_mut(|(cell, slot, task)| match check_ready(task) {
            Some(mesh) => {
                meshed.push((*cell, *slot, mesh));
                false
            }
            None => true,
        });
    for (cell, slot, mesh) in meshed {
        loader.pending[cell] = false;
        let here = loader.distance(cell);
        let mut pending = mesh;
        loop {
            match loader.place(pending, slot, &mut cave) {
                Ok(placement) => {
                    if !placement.sections.1.is_empty() {
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
                        loader.deferred[cell] = true;
                        loader.free_slots.push(slot);
                        break;
                    }
                },
            }
        }
    }

    loader.sweep();
    if loader.dirty
        && let Some(placement) = loader.flush()
    {
        loader.dirty = false;
        loader.uploads.push(Upload::Geometry(placement));
    }

    while loader.parsing.len() < FILES_IN_FLIGHT
        && let Some((coords, path)) = loader.to_parse.pop()
    {
        loader
            .parsing
            .push((coords, pool.spawn(async move { anvil::load(&path) })));
    }

    let room = SECTIONS_IN_FLIGHT.saturating_sub(loader.meshing.len());
    for cell in loader.wanted(room.min(SECTIONS_PER_FRAME)) {
        let Some(slot) = loader.take_slot() else {
            break;
        };
        let world = loader.world.clone();
        let blocks = loader.blocks.clone();
        let section = loader.section_at(cell);
        loader.pending[cell] = true;
        loader.meshing.push((
            cell,
            slot,
            pool.spawn(async move {
                let mut scratch = Scratch::new();
                mesh::mesh_section(&world, &blocks, section, slot, &mut scratch)
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

/// The pack is read once, off the asset system, before anything can bake against it.
fn poll_pack(loader: &mut Loader, assets: &AssetServer) -> Option<Arc<Pack>> {
    match &mut loader.pack {
        PackLoad::Pending => {
            let assets = assets.clone();
            loader.pack = PackLoad::Loading(
                IoTaskPool::get().spawn(async move { Pack::load(&assets).await }),
            );
            None
        }
        PackLoad::Loading(task) => {
            let pack = check_ready(task)?
                .unwrap_or_else(|reason| panic!("cannot read the resource pack: {reason}"));
            println!("{} resource pack files read", pack.len());
            let pack = Arc::new(pack);
            loader.pack = PackLoad::Ready(pack.clone());
            Some(pack)
        }
        PackLoad::Ready(pack) => Some(pack.clone()),
    }
}

fn settle(loader: &mut Loader, pack: &Arc<Pack>, pool: &'static AsyncComputeTaskPool) {
    if loader.baking.is_some() {
        return;
    }
    let catalog = loader.catalog.as_ref().expect("nothing is baking");
    match next_step(
        catalog.blocks.len(),
        loader.palette.states.len(),
        Arc::ptr_eq(&loader.world, &loader.newest),
    ) {
        Next::Bake => start_baking(loader, pack, pool),
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

fn start_baking(loader: &mut Loader, pack: &Arc<Pack>, pool: &'static AsyncComputeTaskPool) {
    let Some(mut catalog) = loader.catalog.take() else {
        return;
    };
    let states = loader.palette.states.clone();
    let biomes = loader.palette.biomes.clone();
    let known = loader.sprites;
    let world = loader.newest.clone();
    let tints = std::mem::take(&mut loader.to_tint);
    let pack = pack.clone();
    let task = pool.spawn(async move {
        blocks::extend(&pack, &mut catalog, &states, &biomes);
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
        let list = &loader.lists[stream];
        let quads: u64 = list.iter().map(|group| u64::from(group.quad_count)).sum();
        println!(
            "  {:<22} {quads:>9} quads in {:>6} groups",
            STREAM_NAMES[stream],
            list.len(),
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
    use crate::anvil::REGION_CHUNKS;

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

    fn loader() -> Loader {
        Loader::new(
            &Budget {
                quads: 1 << 12,
                models: 1 << 12,
                faces: 1 << 16,
                groups: 1 << 12,
                sections: 1 << 8,
                visible: 1 << 12,
                tint_origin: [0; 2],
                tint_size: [1; 2],
            },
            Uploads::default(),
            Window {
                min_region: [0; 2],
                regions: [1; 2],
                files: Vec::new(),
            },
        )
    }

    fn one_greedy_group(section: [usize; 3], slot: u32, quads: u32) -> SectionMesh {
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
                section: slot,
                face: 0,
                quad_prefix: 0,
            }],
            spans,
            connectivity: crate::mesh::CONNECT_ALL,
        }
    }

    #[test]
    fn one_draw_a_bucket_covers_every_section_placed_in_it() {
        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8);

        let near = loader
            .place(one_greedy_group([0, 0, 0], 0, 3), 0, &mut cave)
            .unwrap_or_else(|_| panic!("the arena has room"));
        let far = loader
            .place(one_greedy_group([1, 0, 0], 1, 5), 1, &mut cave)
            .unwrap_or_else(|_| panic!("the arena has room"));
        assert_ne!(
            near.quads.0, far.quads.0,
            "two sections cannot share arena room"
        );
        assert_ne!(
            near.sections.0, far.sections.0,
            "nor a row of the section table"
        );

        let flushed = loader.flush().expect("the group arena has room");
        let draws = flushed
            .draws
            .expect("a flush hands over the whole draw list");
        assert_eq!(
            draws.len(),
            STREAMS,
            "one draw a bucket, however many sections"
        );
        assert_eq!(draws[0].group_count, 2);
        assert_eq!(draws[0].quad_count, 8);
        assert_eq!(
            flushed
                .groups
                .1
                .iter()
                .map(|g| g.quad_prefix)
                .collect::<Vec<_>>(),
            [0, 3],
            "the blend order of a bucket runs across the sections in it"
        );

        let cell = loader.cell([0, 0, 0]);
        loader.evict(cell, &mut cave);
        loader.sweep();
        let flushed = loader.flush().expect("the group arena has room");
        let draws = flushed
            .draws
            .expect("a flush hands over the whole draw list");
        assert_eq!(draws[0].group_count, 1);
        assert_eq!(draws[0].quad_count, 5);
        assert_eq!(
            flushed.groups.1[0].quad_prefix, 0,
            "what the evicted section held is given back, not left as a hole"
        );
    }

    #[test]
    fn a_section_on_a_file_corner_reads_the_diagonal_file_too() {
        let mut files = files_read([-1, -1], 0, 0);
        files.sort();
        assert_eq!(files, [[-2, -2], [-2, -1], [-1, -2], [-1, -1]]);

        let mut files = files_read([-1, -1], REGION_CHUNKS, REGION_CHUNKS);
        files.sort();
        assert_eq!(files, [[-1, -1], [-1, 0], [0, -1], [0, 0]]);
    }
}
