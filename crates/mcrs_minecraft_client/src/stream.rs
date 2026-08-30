use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures::check_ready};
use mcrs_minecraft_network::client::ReceivedRegistries;
use mcrs_minecraft_network::columns::{ColumnStore, SECTION_SIZE};
use mcrs_minecraft_world::block::definition::{BlockDefinitions, Blocks};
use mcrs_voxel_math::ColumnPos;

use crate::arena::{Arena, Block};
use crate::blocks::{self, BlockInfo, Catalog};
use crate::cave::{CaveCull, NO_SLOT};
use crate::mesh::{self, Draw, Group, STREAMS, Scratch, SectionMesh};
use crate::model::Pack;
use crate::pack::QUAD_WORDS;
use crate::render::{Animation, Atlas, Budget, Placement, SectionDesc, Upload, Uploads};

const HYSTERESIS: f32 = (16 * SECTION_SIZE) as f32;

const SECTIONS_IN_FLIGHT: usize = 128;

const SECTIONS_PER_FRAME: usize = 32;

const BIOME_REGISTRY: &str = "minecraft:worldgen/biome";

#[derive(Resource)]
pub struct Loader {
    uploads: Uploads,
    pack: PackLoad,
    catalog: Option<Catalog>,
    blocks: Arc<Vec<BlockInfo>>,
    store: Arc<ColumnStore>,
    known: HashSet<ColumnPos>,
    sections_total: usize,
    baked: Vec<bool>,
    to_bake: Vec<u16>,
    baking: Option<Task<Baked>>,
    foreign_states: bool,
    failures: usize,
    biomes: Vec<String>,
    to_tint: Vec<ColumnPos>,
    tinting: Vec<([u32; 2], Task<Vec<u8>>)>,
    tint_origin: [i32; 2],
    tint_size: [u32; 2],
    resident: HashMap<[i32; 3], Resident>,
    pending: HashSet<[i32; 3]>,
    deferred: HashSet<[i32; 3]>,
    meshing: Vec<([i32; 3], u32, Task<SectionMesh>)>,
    lists: [Vec<Group>; STREAMS],
    group_block: Block,
    slots: usize,
    free_slots: Vec<u32>,
    dead: Vec<u32>,
    slots_used: u32,
    dirty: bool,
    camera: Vec3,
    anchor: Vec3,
    evicted: usize,
    quads: Arena,
    models: Arena,
    faces: Arena,
    groups: Arena,
    sprites: usize,
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
    pub columns: usize,
    pub sections: usize,
    pub sections_total: usize,
    pub evicted: usize,
    pub quads: f32,
    pub models: f32,
}

impl Loader {
    pub fn new(budget: &Budget, uploads: Uploads) -> Self {
        Self {
            uploads,
            pack: PackLoad::Pending,
            catalog: Some(blocks::empty()),
            blocks: Arc::new(Vec::new()),
            store: Arc::new(ColumnStore::default()),
            known: HashSet::new(),
            sections_total: 0,
            baked: Vec::new(),
            to_bake: Vec::new(),
            baking: None,
            foreign_states: false,
            failures: 0,
            biomes: Vec::new(),
            to_tint: Vec::new(),
            tinting: Vec::new(),
            tint_origin: budget.tint_origin,
            tint_size: budget.tint_size,
            resident: HashMap::new(),
            pending: HashSet::new(),
            deferred: HashSet::new(),
            meshing: Vec::new(),
            lists: std::array::from_fn(|_| Vec::new()),
            group_block: Block::EMPTY,
            slots: budget.sections,
            free_slots: Vec::new(),
            dead: Vec::new(),
            slots_used: 0,
            dirty: false,
            camera: Vec3::ZERO,
            anchor: Vec3::splat(f32::MAX),
            evicted: 0,
            quads: Arena::new(budget.quads),
            models: Arena::new(budget.models),
            faces: Arena::new(budget.faces),
            groups: Arena::new(budget.groups),
            sprites: 0,
        }
    }

    pub fn status(&self) -> Status {
        Status {
            columns: self.known.len(),
            sections: self.resident.len(),
            sections_total: self.sections_total,
            evicted: self.evicted,
            quads: self.quads.held() as f32 / self.quads.capacity() as f32,
            models: self.models.held() as f32 / self.models.capacity() as f32,
        }
    }

    pub fn done(&self) -> bool {
        self.idle() && self.uploads.waiting() == 0
    }

    fn idle(&self) -> bool {
        self.to_bake.is_empty()
            && self.baking.is_none()
            && self.to_tint.is_empty()
            && self.tinting.is_empty()
            && self.meshing.is_empty()
            && !self.dirty
            && self.wanted(1).is_empty()
    }

    /// The catalog has to reach every state a resident column names before a
    /// section holding one can be meshed against it.
    fn caught_up(&self) -> bool {
        self.to_bake.is_empty() && self.baking.is_none() && !self.blocks.is_empty()
    }

    fn distance(&self, section: [i32; 3]) -> f32 {
        let min = Vec3::from_array(section.map(|n| (n * SECTION_SIZE as i32) as f32));
        let max = min + Vec3::splat(SECTION_SIZE as f32);
        (self.camera.clamp(min, max) - self.camera).length()
    }

    /// A section reads one block past its own faces, so it borders the eight
    /// columns around its own and cannot be meshed until they have arrived.
    fn surrounded(&self, pos: ColumnPos) -> bool {
        (-1..=1).all(|dz| {
            (-1..=1).all(|dx| self.store.holds(ColumnPos::new(pos.x + dx, pos.z + dz)))
        })
    }

    /// The nearest sections that hold blocks, have nowhere to be but the arena, and border only
    /// columns that have arrived. Nothing more than distance decides the order yet.
    fn wanted(&self, want: usize) -> Vec<[i32; 3]> {
        let Some(extent) = self.store.extent() else {
            return Vec::new();
        };
        if want == 0 {
            return Vec::new();
        }
        let mut nearest: Vec<(f32, [i32; 3])> = Vec::new();
        for pos in self.store.positions() {
            if !self.surrounded(pos) {
                continue;
            }
            for step in 0..extent.sections {
                let at = [pos.x, extent.min_section_y + step as i32, pos.z];
                if self.resident.contains_key(&at)
                    || self.pending.contains(&at)
                    || self.deferred.contains(&at)
                    || self.store.section(at[0], at[1], at[2]).is_none()
                {
                    continue;
                }
                nearest.push((self.distance(at), at));
            }
        }
        if nearest.len() > want {
            nearest.select_nth_unstable_by(want, |a, b| a.0.total_cmp(&b.0));
            nearest.truncate(want);
        }
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        nearest.into_iter().map(|(_, at)| at).collect()
    }

    fn victim(&self, candidate: f32) -> Option<[i32; 3]> {
        let farthest = *self
            .resident
            .keys()
            .max_by(|a, b| self.distance(**a).total_cmp(&self.distance(**b)))?;
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

    fn evict(&mut self, section: [i32; 3], cave: &mut CaveCull) {
        let Some(held) = self.resident.remove(&section) else {
            return;
        };
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.faces.free(held.faces);
        if held.slot != NO_SLOT {
            self.dead.push(held.slot);
            self.dirty = true;
        }
        cave.forget(section);
        self.evicted += 1;
        self.deferred.clear();
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
        let held: Vec<([i32; 3], u32, u64)> = self
            .resident
            .iter()
            .map(|(at, held)| (*at, held.slot, held.connectivity))
            .collect();
        for (at, slot, mask) in held {
            cave.set_section(at, slot, mask);
        }
    }

    /// Takes the store's newest shape: the sections of a column the server has
    /// taken back go with it, and a column that has just arrived is walked for
    /// the block states the catalog still owes and queued for its tints.
    ///
    /// ponytail: a column the server sends a second time keeps the geometry
    /// meshed from the first, so a block change never reaches the screen; the
    /// upgrade is to compare what is resident against the store's own columns
    /// rather than a set of positions.
    fn adopt(&mut self, store: &ColumnStore, definitions: &BlockDefinitions, cave: &mut CaveCull) {
        let extent = store.extent();
        let departed: Vec<ColumnPos> = self
            .known
            .iter()
            .copied()
            .filter(|pos| !store.holds(*pos))
            .collect();
        for pos in departed {
            self.known.remove(&pos);
            self.to_tint.retain(|queued| *queued != pos);
            if let Some(extent) = self.store.extent() {
                for step in 0..extent.sections {
                    let at = [pos.x, extent.min_section_y + step as i32, pos.z];
                    if self.store.section(at[0], at[1], at[2]).is_some() {
                        self.sections_total -= 1;
                    }
                    self.evict(at, cave);
                }
            }
        }

        let arrived: Vec<ColumnPos> = store
            .positions()
            .filter(|pos| !self.known.contains(pos))
            .collect();
        self.store = Arc::new(store.clone());
        if self.baked.len() < definitions.state_count() {
            self.baked.resize(definitions.state_count(), false);
        }
        for pos in arrived {
            self.known.insert(pos);
            self.to_tint.push(pos);
            let Some(extent) = extent else { continue };
            for step in 0..extent.sections {
                let sy = extent.min_section_y + step as i32;
                let Some(section) = self.store.section(pos.x, sy, pos.z) else {
                    continue;
                };
                self.sections_total += 1;
                for &state in section.blocks.iter() {
                    match self.baked.get_mut(state as usize) {
                        Some(true) => {}
                        Some(seen) => {
                            *seen = true;
                            self.to_bake.push(state);
                        }
                        None if self.foreign_states => {}
                        None => {
                            self.foreign_states = true;
                            error!(
                                state,
                                "the server names block states this corpus has no definition \
                                 for; they are drawn as air"
                            );
                        }
                    }
                }
            }
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

        let section = mesh.section;
        let slot = if placed.is_empty() {
            self.free_slots.push(slot);
            NO_SLOT
        } else {
            self.dirty = true;
            slot
        };
        cave.set_section(section, slot, mesh.connectivity);
        self.resident.insert(
            section,
            Resident {
                quads,
                models,
                faces,
                slot,
                connectivity: mesh.connectivity,
            },
        );

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

    /// Where a column's tint square sits in the tint texture, or `None` when
    /// the column falls outside the window the texture covers.
    fn tint_corner(&self, pos: ColumnPos) -> Option<[u32; 2]> {
        let span = SECTION_SIZE as u32;
        let corner = [
            pos.x * SECTION_SIZE as i32 - self.tint_origin[0],
            pos.z * SECTION_SIZE as i32 - self.tint_origin[1],
        ];
        let inside = |axis: usize| {
            u32::try_from(corner[axis])
                .ok()
                .filter(|near| near + span <= self.tint_size[axis])
        };
        Some([inside(0)?, inside(1)?])
    }
}

fn worth_evicting(resident: f32, candidate: f32) -> bool {
    resident > candidate + HYSTERESIS
}

pub fn advance(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
    assets: Res<AssetServer>,
    definitions: Res<Blocks>,
    store: Option<Res<ColumnStore>>,
    registries: Query<&ReceivedRegistries>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let Some(store) = store else {
        return;
    };
    let pool = AsyncComputeTaskPool::get();
    let loader = &mut *loader;
    let pack = poll_pack(loader, &assets);
    loader.camera = camera.translation();
    loader.follow(&mut cave);
    if loader.camera.distance(loader.anchor) > HYSTERESIS {
        loader.anchor = loader.camera;
        loader.deferred.clear();
    }

    if loader.biomes.is_empty() {
        loader.biomes = biome_names(&registries);
    }
    if store.is_changed() {
        loader.adopt(&store, &definitions, &mut cave);
    }

    if let Some(task) = loader.baking.as_mut()
        && let Some(baked) = check_ready(task)
    {
        loader.baking = None;
        publish(loader, baked);
    }
    if let Some(pack) = pack
        && loader.baking.is_none()
        && !loader.to_bake.is_empty()
        && !loader.biomes.is_empty()
    {
        start_baking(loader, &pack, &definitions, pool);
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
            size: SECTION_SIZE as u32,
            data,
        });
    }
    start_tinting(loader, pool);

    let mut meshed = Vec::new();
    loader
        .meshing
        .retain_mut(|(at, slot, task)| match check_ready(task) {
            Some(mesh) => {
                meshed.push((*at, *slot, mesh));
                false
            }
            None => true,
        });
    for (at, slot, mesh) in meshed {
        loader.pending.remove(&at);
        let here = loader.distance(at);
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
                        loader.deferred.insert(at);
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

    if !loader.caught_up() {
        return;
    }
    let room = SECTIONS_IN_FLIGHT.saturating_sub(loader.meshing.len());
    for at in loader.wanted(room.min(SECTIONS_PER_FRAME)) {
        let Some(slot) = loader.take_slot() else {
            break;
        };
        let world = loader.store.clone();
        let blocks = loader.blocks.clone();
        loader.pending.insert(at);
        loader.meshing.push((
            at,
            slot,
            pool.spawn(async move {
                let mut scratch = Scratch::new();
                mesh::mesh_section(&world, &blocks, at, slot, &mut scratch)
            }),
        ));
    }
}

fn biome_names(registries: &Query<&ReceivedRegistries>) -> Vec<String> {
    registries
        .iter()
        .flat_map(|received| received.0.iter())
        .find(|registry| registry.registry == BIOME_REGISTRY)
        .map(|registry| {
            registry
                .entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect()
        })
        .unwrap_or_default()
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
            info!(files = pack.len(), "read the resource pack");
            let pack = Arc::new(pack);
            loader.pack = PackLoad::Ready(pack.clone());
            Some(pack)
        }
        PackLoad::Ready(pack) => Some(pack.clone()),
    }
}

fn start_baking(
    loader: &mut Loader,
    pack: &Arc<Pack>,
    definitions: &Blocks,
    pool: &'static AsyncComputeTaskPool,
) {
    let Some(mut catalog) = loader.catalog.take() else {
        return;
    };
    let states = std::mem::take(&mut loader.to_bake);
    let biomes = loader.biomes.clone();
    let known = loader.sprites;
    let definitions = definitions.clone();
    let pack = pack.clone();
    loader.baking = Some(pool.spawn(async move {
        blocks::extend(&pack, &mut catalog, &definitions, &states, &biomes);
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
    }));
}

fn publish(loader: &mut Loader, baked: Baked) {
    if let Some(sprites) = baked.sprites {
        loader.sprites = baked.catalog.sprites.len();
        loader.uploads.push(sprites);
    }
    for failure in &baked.catalog.failures[loader.failures..] {
        warn!("no model for {failure}");
    }
    loader.failures = baked.catalog.failures.len();
    loader.catalog = Some(baked.catalog);
    loader.blocks = Arc::new(baked.blocks);
}

fn start_tinting(loader: &mut Loader, pool: &'static AsyncComputeTaskPool) {
    let Some(catalog) = loader.catalog.as_ref() else {
        return;
    };
    if catalog.tints.len() <= 1 {
        return;
    }
    let tints = catalog.tints.clone();
    for pos in std::mem::take(&mut loader.to_tint) {
        let Some(corner) = loader.tint_corner(pos) else {
            continue;
        };
        let world = loader.store.clone();
        let tints = tints.clone();
        loader
            .tinting
            .push((corner, pool.spawn(async move {
                blocks::tint_column(&world, &tints, pos)
            })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::StreamSpan;

    fn loader() -> Loader {
        Loader::new(
            &Budget {
                quads: 1 << 12,
                models: 1 << 12,
                faces: 1 << 16,
                groups: 1 << 12,
                sections: 1 << 8,
                visible: 1 << 12,
                tint_origin: [-256, -256],
                tint_size: [512; 2],
            },
            Uploads::default(),
        )
    }

    fn one_greedy_group(section: [i32; 3], slot: u32, quads: u32) -> SectionMesh {
        let mut spans = [Default::default(); STREAMS];
        spans[0] = StreamSpan {
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

        loader.evict([0, 0, 0], &mut cave);
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
    fn a_column_is_meshed_only_once_every_column_it_borders_has_arrived() {
        use mcrs_minecraft_network::columns::{Column, Extent};

        let mut loader = loader();
        let mut store = ColumnStore::default();
        store.enter(Extent {
            min_section_y: 0,
            sections: 1,
        });
        for x in -1..=1 {
            for z in -1..=1 {
                store.insert(ColumnPos::new(x, z), Column::unlit(0, Vec::new()));
            }
        }
        loader.store = Arc::new(store.clone());
        assert!(loader.surrounded(ColumnPos::new(0, 0)));
        assert!(
            !loader.surrounded(ColumnPos::new(1, 0)),
            "an edge column still has three neighbours missing"
        );

        store.remove(ColumnPos::new(-1, -1));
        loader.store = Arc::new(store);
        assert!(
            !loader.surrounded(ColumnPos::new(0, 0)),
            "the diagonal is read too, so losing it is enough"
        );
    }

    #[test]
    fn no_section_is_meshed_before_the_catalog_reaches_the_states_it_holds() {
        let mut loader = loader();
        assert!(!loader.caught_up(), "nothing has been baked yet");

        loader.blocks = Arc::new(vec![BlockInfo::default()]);
        assert!(loader.caught_up());

        loader.to_bake.push(7);
        assert!(
            !loader.caught_up(),
            "a state the catalog still owes holds the mesher back"
        );
    }

    #[test]
    fn a_column_outside_the_tint_window_is_not_written_into_it() {
        let loader = loader();
        assert_eq!(loader.tint_corner(ColumnPos::new(-16, -16)), Some([0, 0]));
        assert_eq!(loader.tint_corner(ColumnPos::new(0, 0)), Some([256, 256]));
        assert_eq!(loader.tint_corner(ColumnPos::new(15, 0)), Some([496, 256]));
        assert_eq!(loader.tint_corner(ColumnPos::new(16, 0)), None);
        assert_eq!(loader.tint_corner(ColumnPos::new(-17, 0)), None);
    }
    /// The chain the renderer now stands on end to end: a real server generates
    /// terrain, sends it as chunk packets, the store keeps what arrives, and a
    /// section of it meshes into quads.
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn a_column_the_server_sends_meshes_into_quads() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        use bevy::app::AppExit;
        use bevy::ecs::message::MessageWriter;
        use mcrs_minecraft_network::client::ClientNetworkPlugin;
        use mcrs_minecraft_server::{
            BoundAddress, MinecraftServerPlugin, run_server_loop, spawn_server_thread,
        };

        use crate::mesh::mesh_section;

        let mut server = App::new();
        server.add_plugins(MinecraftServerPlugin::embedded());
        let address = server.world().resource::<BoundAddress>().0;
        let stop = Arc::new(AtomicBool::new(false));
        let raised = stop.clone();
        server.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
            if raised.load(Ordering::Relaxed) {
                exit.write(AppExit::Success);
            }
        });
        let (stopped, wait) = std::sync::mpsc::channel();
        let thread = spawn_server_thread(server, move |app| {
            run_server_loop(app);
            stopped.send(()).ok();
        });

        let mut client = App::new();
        client.add_plugins(ClientNetworkPlugin {
            server: address,
            username: "mcrs_mesh".to_owned(),
        });

        let deadline = Instant::now() + Duration::from_secs(120);
        let mut solid = None;
        while Instant::now() < deadline && solid.is_none() {
            client.update();
            let store = client.world().resource::<ColumnStore>();
            if let Some(extent) = store.extent() {
                solid = store
                    .positions()
                    .flat_map(|pos| {
                        (0..extent.sections as i32)
                            .map(move |i| [pos.x, extent.min_section_y + i, pos.z])
                    })
                    .max_by_key(|at| {
                        store.section(at[0], at[1], at[2]).map_or(0, |section| {
                            let mut seen: Vec<u16> = Vec::new();
                            for &id in section.blocks.iter() {
                                if id != 0 && !seen.contains(&id) {
                                    seen.push(id);
                                }
                            }
                            seen.len()
                        })
                    })
                    .filter(|at| {
                        store
                            .section(at[0], at[1], at[2])
                            .is_some_and(|section| section.blocks.iter().any(|&id| id != 0))
                    })
                    .map(|at| (at, store.clone()));
            }
            if solid.is_none() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        stop.store(true, Ordering::Relaxed);
        wait.recv_timeout(Duration::from_secs(30))
            .expect("the embedded server did not stop after AppExit");
        thread.join().expect("server thread joined");

        let (section, store) =
            solid.expect("no column the server sent held a section of anything but air");

        let base = section.map(|n| n * SECTION_SIZE as i32);
        let mut states: Vec<u16> = Vec::new();
        for y in -1..=SECTION_SIZE as i32 {
            for z in -1..=SECTION_SIZE as i32 {
                for x in -1..=SECTION_SIZE as i32 {
                    let id = store.block(base[0] + x, base[1] + y, base[2] + z);
                    if id != 0 && !states.contains(&id) {
                        states.push(id);
                    }
                }
            }
        }

        let definitions = blocks::corpus();
        let mut catalog = blocks::empty();
        blocks::extend(Pack::corpus(), &mut catalog, definitions, &states, &[]);
        let baked = states
            .iter()
            .filter(|&&id| {
                let info = &catalog.blocks[id as usize];
                info.cube.is_some() || !info.quads.is_empty()
            })
            .count();
        assert!(
            baked > 0,
            "the catalog baked geometry for none of the {} states the server sent: {:?}",
            states.len(),
            catalog.failures,
        );

        let mesh = mesh_section(&store, &catalog.blocks, section, 0, &mut Scratch::new());
        let quads: u32 = mesh.spans.iter().map(|span| span.quad_count).sum();
        assert!(
            quads > 0,
            "section {section:?} holds {} block states but meshed no quads",
            states.len(),
        );
    }
}
