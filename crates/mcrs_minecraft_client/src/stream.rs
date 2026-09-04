use std::sync::Arc;

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures::check_ready};
use mcrs_minecraft_network::client::ReceivedRegistries;
use mcrs_minecraft_network::columns::{
    BlockSource, ColumnChange, ColumnStore, Extent, SECTION_SIZE,
};
use mcrs_minecraft_world::block::definition::{BlockDefinitions, Blocks};
use mcrs_voxel_math::ColumnPos;
use mcrs_voxel_world::world::lifecycle::trace::{self, ColumnStage};

use crate::arena::{Arena, Block};
use crate::blocks::{self, BlockInfo, Catalog};
use crate::cave::{CaveCull, NO_SLOT};
use crate::mesh::{self, Connectivity, Draw, Group, STREAMS, Scratch, SectionMesh};
use crate::model::Pack;
use crate::pack::QUAD_WORDS;
use crate::render::{Animation, AtlasUpdate, Budget, Placement, SectionDesc, Upload, Uploads};

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
    changes: Vec<ColumnChange>,
    columns: usize,
    sections_total: usize,
    baked: Vec<bool>,
    to_bake: Vec<u16>,
    baking: Option<Task<Baked>>,
    foreign_states: bool,
    failures: usize,
    biomes: Vec<String>,
    to_tint: Vec<ColumnPos>,
    tinting: Vec<([u32; 2], Task<Vec<u8>>)>,
    tint_size: [u32; 2],
    resident: HashMap<[i32; 3], Resident>,
    pending: HashSet<[i32; 3]>,
    deferred: HashSet<[i32; 3]>,
    queue: MeshQueue,
    meshing: Vec<([i32; 3], u32, Task<(SectionMesh, Scratch)>)>,
    scratches: Vec<Scratch>,
    /// Every stream's group records as the GPU holds them, dead ones kept as records with no
    /// quads until the stream is rebuilt, so a change only ever writes the records it touched.
    lists: [Vec<Group>; STREAMS],
    group_blocks: [Block; STREAMS],
    quads_end: [u32; STREAMS],
    dead_records: [usize; STREAMS],
    touched: [Vec<(u32, u32)>; STREAMS],
    rebuild: [bool; STREAMS],
    owners: Vec<[i32; 3]>,
    slots: usize,
    free_slots: Vec<u32>,
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
    sent: Vec<(u32, u32)>,
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
    connectivity: Connectivity,
    /// Where this section's records sit in each stream's list: first index and count.
    groups: [(u32, u32); STREAMS],
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
    pub faces: f32,
    pub groups: f32,
    pub queued: usize,
    pub meshing: usize,
    pub uploads_waiting: usize,
}

impl Loader {
    pub fn new(budget: &Budget, uploads: Uploads) -> Self {
        Self {
            uploads,
            pack: PackLoad::Pending,
            catalog: Some(blocks::empty()),
            blocks: Arc::new(Vec::new()),
            changes: Vec::new(),
            columns: 0,
            sections_total: 0,
            baked: Vec::new(),
            to_bake: Vec::new(),
            baking: None,
            foreign_states: false,
            failures: 0,
            biomes: Vec::new(),
            to_tint: Vec::new(),
            tinting: Vec::new(),
            tint_size: budget.tint_size,
            resident: HashMap::new(),
            pending: HashSet::new(),
            deferred: HashSet::new(),
            queue: MeshQueue::default(),
            meshing: Vec::new(),
            scratches: Vec::new(),
            lists: std::array::from_fn(|_| Vec::new()),
            group_blocks: std::array::from_fn(|_| Block::EMPTY),
            quads_end: [0; STREAMS],
            dead_records: [0; STREAMS],
            touched: std::array::from_fn(|_| Vec::new()),
            rebuild: [false; STREAMS],
            owners: vec![[0; 3]; budget.sections],
            slots: budget.sections,
            free_slots: Vec::new(),
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
            sent: Vec::new(),
        }
    }

    pub fn status(&self) -> Status {
        Status {
            columns: self.columns,
            sections: self.resident.len(),
            sections_total: self.sections_total,
            evicted: self.evicted,
            quads: self.quads.held() as f32 / self.quads.capacity() as f32,
            models: self.models.held() as f32 / self.models.capacity() as f32,
            faces: self.faces.held() as f32 / self.faces.capacity() as f32,
            groups: self.groups.held() as f32 / self.groups.capacity() as f32,
            queued: self.queue.len(),
            meshing: self.meshing.len(),
            uploads_waiting: self.uploads.waiting(),
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
            && self.queue.is_empty()
    }

    /// The catalog has to reach every state a resident column names before a
    /// section holding one can be meshed against it.
    fn caught_up(&self) -> bool {
        self.to_bake.is_empty() && self.baking.is_none() && !self.blocks.is_empty()
    }

    fn distance(&self, section: [i32; 3]) -> f32 {
        distance_from(self.camera, section)
    }

    /// A section reads one block past its own faces, so it borders the eight
    /// columns around its own and cannot be meshed until they have arrived.
    fn surrounded(&self, pos: ColumnPos, store: &ColumnStore) -> bool {
        (-1..=1).all(|dz| (-1..=1).all(|dx| store.holds(ColumnPos::new(pos.x + dx, pos.z + dz))))
    }

    /// Holds blocks, has nowhere to be but the arena, and borders only columns that have
    /// arrived.
    fn meshable(&self, at: [i32; 3], store: &ColumnStore) -> bool {
        !self.resident.contains_key(&at)
            && !self.pending.contains(&at)
            && store.section(at[0], at[1], at[2]).is_some()
            && self.surrounded(ColumnPos::new(at[0], at[2]), store)
    }

    fn camera_section(&self) -> [i32; 3] {
        (self.camera / SECTION_SIZE as f32)
            .floor()
            .as_ivec3()
            .to_array()
    }

    fn enqueue(&mut self, at: [i32; 3]) {
        let camera = self.camera_section();
        self.queue.insert(at, camera);
    }

    /// A section turns meshable when the last of the nine columns it reads arrives, which is
    /// as often a neighbour of its own column as the column itself.
    fn enqueue_around(&mut self, pos: ColumnPos, extent: Extent, store: &ColumnStore) {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let column = ColumnPos::new(pos.x + dx, pos.z + dz);
                if !self.surrounded(column, store) {
                    continue;
                }
                for step in 0..extent.sections {
                    let at = [column.x, extent.min_section_y + step as i32, column.z];
                    if self.meshable(at, store) {
                        self.enqueue(at);
                    }
                }
            }
        }
    }

    fn requeue_deferred(&mut self) {
        for at in std::mem::take(&mut self.deferred) {
            self.enqueue(at);
        }
    }

    /// The nearest queued sections. Nothing more than distance decides the order yet.
    fn take_wanted(&mut self, want: usize, store: &ColumnStore) -> Vec<[i32; 3]> {
        if want == 0 || self.queue.is_empty() {
            return Vec::new();
        }
        let camera = self.camera_section();
        let mut taken = Vec::with_capacity(want);
        while taken.len() < want {
            let Some(at) = self.queue.pop_nearest(camera) else {
                break;
            };
            if self.meshable(at, store) {
                taken.push(at);
            }
        }
        taken
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
            for stream in 0..STREAMS {
                let (first, count) = held.groups[stream];
                if count == 0 {
                    continue;
                }
                let (first, end) = (first as usize, (first + count) as usize);
                for group in &mut self.lists[stream][first..end] {
                    group.quad_count = 0;
                }
                self.dead_records[stream] += count as usize;
                self.touched[stream].push((first as u32, end as u32));
                if self.dead_records[stream] * 2 >= self.lists[stream].len() {
                    self.rebuild[stream] = true;
                }
            }
            self.free_slots.push(held.slot);
            self.dirty = true;
        }
        cave.forget(section);
        self.evicted += 1;
        self.requeue_deferred();
    }

    fn follow(&mut self, cave: &mut CaveCull) {
        if !cave.follow(self.camera) {
            return;
        }
        let held: Vec<([i32; 3], u32, Connectivity)> = self
            .resident
            .iter()
            .map(|(at, held)| (*at, held.slot, held.connectivity))
            .collect();
        for (at, slot, mask) in held {
            cave.set_section(at, slot, mask);
        }
    }

    /// Takes what the server sent and took back since the last frame: a departed column's
    /// sections leave the queue and the arena with it, and an arrived column is walked for the
    /// block states the catalog still owes and queued for its tints.
    fn adopt(&mut self, store: &ColumnStore, definitions: &BlockDefinitions, cave: &mut CaveCull) {
        let extent = store.extent();
        if self.baked.len() < definitions.state_count() {
            self.baked.resize(definitions.state_count(), false);
        }
        let mut changes = std::mem::take(&mut self.changes);
        for change in changes.drain(..) {
            match change {
                ColumnChange::Departed(pos, column) => {
                    trace::forget(pos);
                    self.columns -= 1;
                    self.to_tint.retain(|queued| *queued != pos);
                    for (sy, section) in column.sections() {
                        if section.is_some() {
                            self.sections_total -= 1;
                        }
                        let at = [pos.x, sy, pos.z];
                        self.queue.remove(at);
                        self.evict(at, cave);
                    }
                }
                ColumnChange::Arrived(pos) => {
                    trace::mark(pos, ColumnStage::Received);
                    self.columns += 1;
                    self.to_tint.push(pos);
                    let Some(extent) = extent else { continue };
                    for step in 0..extent.sections {
                        let sy = extent.min_section_y + step as i32;
                        let Some(section) = store.section(pos.x, sy, pos.z) else {
                            continue;
                        };
                        self.sections_total += 1;
                        for &state in &section.states {
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
                                        "the server names block states this corpus has no \
                                         definition for; they are drawn as air"
                                    );
                                }
                            }
                        }
                    }
                    self.enqueue_around(pos, extent, store);
                }
            }
        }
        self.changes = changes;
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
        let mut ranges = [(0u32, 0u32); STREAMS];
        for stream in 0..STREAMS {
            let run = mesh.spans[stream].group_count as usize;
            let base = if stream % 2 == 0 {
                quads.offset
            } else {
                models.offset
            } as u32;
            for group in &mut placed[first..first + run] {
                group.quad_base += base;
                group.quad_prefix = self.quads_end[stream];
                self.quads_end[stream] += group.quad_count;
            }
            if run != 0 {
                let at = self.lists[stream].len() as u32;
                self.lists[stream].extend_from_slice(&placed[first..first + run]);
                ranges[stream] = (at, run as u32);
                self.touched[stream].push((at, at + run as u32));
                if self.lists[stream].len() > self.group_blocks[stream].capacity() {
                    self.rebuild[stream] = true;
                }
            }
            first += run;
        }

        let section = mesh.section;
        let slot = if placed.is_empty() {
            self.free_slots.push(slot);
            NO_SLOT
        } else {
            self.dirty = true;
            self.owners[slot as usize] = section;
            slot
        };
        cave.set_section(section, slot, mesh.connectivity);
        trace::mark(ColumnPos::new(section[0], section[2]), ColumnStage::Meshed);
        self.resident.insert(
            section,
            Resident {
                quads,
                models,
                faces,
                slot,
                connectivity: mesh.connectivity,
                groups: ranges,
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

    /// Writes the records each stream touched since the last flush, rebuilding a stream into a
    /// fresh block first when it outgrew its block or half of it is dead. A stream that cannot
    /// get a block draws what its block holds and tries again next time.
    fn flush(&mut self) -> Option<Placement> {
        let mut groups = Vec::new();
        for stream in 0..STREAMS {
            if self.rebuild[stream] {
                self.rebuild_stream(stream);
            }
            let capacity = self.group_blocks[stream].capacity() as u32;
            let mut ranges = std::mem::take(&mut self.touched[stream]);
            ranges.sort_unstable();
            let mut merged: Vec<(u32, u32)> = Vec::new();
            for (first, end) in ranges.drain(..) {
                let end = end.min(capacity);
                if first >= end {
                    continue;
                }
                match merged.last_mut() {
                    Some(last) if first <= last.1 + RUN_GAP => last.1 = last.1.max(end),
                    _ => merged.push((first, end)),
                }
            }
            if self.rebuild[stream] {
                // What lies past the block is written once the rebuild goes through.
                self.touched[stream].push((capacity, self.lists[stream].len() as u32));
            }
            for (first, end) in merged {
                let offset =
                    (self.group_blocks[stream].offset + first as usize) * size_of::<Group>();
                groups.push((
                    offset as u64,
                    self.lists[stream][first as usize..end as usize].to_vec(),
                ));
            }
        }
        let draws = (0..STREAMS)
            .map(|stream| Draw {
                stream: stream as u32,
                first_group: self.group_blocks[stream].offset as u32,
                group_count: self.lists[stream]
                    .len()
                    .min(self.group_blocks[stream].capacity()) as u32,
                quad_count: self.quads_end[stream],
            })
            .collect();
        Some(Placement {
            groups,
            draws: Some(draws),
            ..Placement::default()
        })
    }

    /// Packs a stream's live records into a fresh block with room to grow, keeping their order
    /// and telling each resident section where its records went.
    fn rebuild_stream(&mut self, stream: usize) {
        let live = self.lists[stream].len() - self.dead_records[stream];
        let roomy = (live * 2).next_power_of_two().max(MIN_BLOCK);
        let tight = live.next_power_of_two().max(MIN_BLOCK);
        let Some(block) = self
            .groups
            .alloc(roomy)
            .or_else(|| self.groups.alloc(tight))
        else {
            warn!(
                stream,
                live,
                held = self.groups.held(),
                capacity = self.groups.capacity(),
                "the group arena cannot hold a rebuild; the stream draws only what its block \
                 holds and the rest of its sections stay off the screen"
            );
            return;
        };
        let stale = std::mem::replace(&mut self.group_blocks[stream], block);
        self.groups.free(stale);
        let old = std::mem::take(&mut self.lists[stream]);
        let mut packed = Vec::with_capacity(live);
        let mut quads = 0u32;
        let mut last_slot = NO_SLOT;
        for mut group in old.into_iter().filter(|group| group.quad_count != 0) {
            let owner = self.owners[group.section as usize];
            if let Some(resident) = self.resident.get_mut(&owner) {
                if group.section != last_slot {
                    resident.groups[stream] = (packed.len() as u32, 0);
                }
                resident.groups[stream].1 += 1;
            }
            last_slot = group.section;
            group.quad_prefix = quads;
            quads += group.quad_count;
            packed.push(group);
        }
        self.lists[stream] = packed;
        self.quads_end[stream] = quads;
        self.dead_records[stream] = 0;
        self.touched[stream].clear();
        self.touched[stream].push((0, live as u32));
        self.rebuild[stream] = false;
    }

    /// Where a column's tint square sits in the tint texture, which the world wraps into.
    fn tint_corner(&self, pos: ColumnPos) -> [u32; 2] {
        let wrapped = |axis: i32, size: u32| (axis * SECTION_SIZE as i32).rem_euclid(size as i32);
        [
            wrapped(pos.x, self.tint_size[0]) as u32,
            wrapped(pos.z, self.tint_size[1]) as u32,
        ]
    }
}

/// Touched runs of records this close together are written as one copy.
const RUN_GAP: u32 = 64;
/// The smallest block a stream is rebuilt into, in records.
const MIN_BLOCK: usize = 256;

/// Sections waiting to be meshed, bucketed by how many sections they lie from a reference
/// position along their farthest axis, so the nearest come out first without ever sorting the
/// whole queue. The reference follows the camera in steps of `QUEUE_STEP` sections, and each
/// step rebuckets everything, so the order is never staler than that.
#[derive(Default)]
struct MeshQueue {
    buckets: Vec<Vec<[i32; 3]>>,
    queued: HashSet<[i32; 3]>,
    reference: [i32; 3],
    /// Entries across the buckets, those since removed from the set included.
    entries: usize,
    scratch: Vec<[i32; 3]>,
}

/// Buckets past this distance share the last one.
const QUEUE_BUCKETS: usize = 256;
/// How far the camera moves from the reference before the queue is rebucketed.
const QUEUE_STEP: usize = 8;

impl MeshQueue {
    fn bucket(at: [i32; 3], from: [i32; 3]) -> usize {
        let axis = |a: i32, c: i32| a.abs_diff(c) as usize;
        axis(at[0], from[0])
            .max(axis(at[1], from[1]))
            .max(axis(at[2], from[2]))
            .min(QUEUE_BUCKETS - 1)
    }

    fn insert(&mut self, at: [i32; 3], camera: [i32; 3]) {
        if !self.queued.insert(at) {
            return;
        }
        if self.buckets.is_empty() {
            self.buckets = (0..QUEUE_BUCKETS).map(|_| Vec::new()).collect();
            self.reference = camera;
        }
        self.buckets[Self::bucket(at, self.reference)].push(at);
        self.entries += 1;
    }

    fn remove(&mut self, at: [i32; 3]) {
        self.queued.remove(&at);
    }

    fn len(&self) -> usize {
        self.queued.len()
    }

    fn is_empty(&self) -> bool {
        self.queued.is_empty()
    }

    #[cfg(test)]
    fn contains(&self, at: &[i32; 3]) -> bool {
        self.queued.contains(at)
    }

    /// The queued section nearest the camera, taken out of the queue.
    fn pop_nearest(&mut self, camera: [i32; 3]) -> Option<[i32; 3]> {
        if self.queued.is_empty() {
            return None;
        }
        let moved = Self::bucket(camera, self.reference) >= QUEUE_STEP;
        if moved || self.entries > 2 * self.queued.len() + 1024 {
            self.rebucket(camera);
        }
        for bucket in &mut self.buckets {
            while let Some(at) = bucket.pop() {
                self.entries -= 1;
                if self.queued.remove(&at) {
                    return Some(at);
                }
            }
        }
        None
    }

    /// Buckets every live entry afresh around the camera, dropping the entries whose sections
    /// have since left the queue.
    fn rebucket(&mut self, camera: [i32; 3]) {
        self.scratch.clear();
        for bucket in &mut self.buckets {
            self.scratch
                .extend(bucket.drain(..).filter(|at| self.queued.contains(at)));
        }
        self.reference = camera;
        for at in self.scratch.drain(..) {
            self.buckets[Self::bucket(at, camera)].push(at);
        }
        self.entries = self.queued.len();
    }
}

fn worth_evicting(resident: f32, candidate: f32) -> bool {
    resident > candidate + HYSTERESIS
}

fn distance_from(camera: Vec3, section: [i32; 3]) -> f32 {
    let min = Vec3::from_array(section.map(|n| (n * SECTION_SIZE as i32) as f32));
    let max = min + Vec3::splat(SECTION_SIZE as f32);
    (camera.clamp(min, max) - camera).length()
}

pub fn advance(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
    assets: Res<AssetServer>,
    definitions: Res<Blocks>,
    store: Option<ResMut<ColumnStore>>,
    registries: Query<&ReceivedRegistries>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let Some(mut store) = store else {
        return;
    };
    let store = store.bypass_change_detection();
    let pool = AsyncComputeTaskPool::get();
    let loader = &mut *loader;
    let pack = poll_pack(loader, &assets);
    loader.camera = camera.translation();
    loader.follow(&mut cave);
    if loader.camera.distance(loader.anchor) > HYSTERESIS {
        loader.anchor = loader.camera;
        loader.requeue_deferred();
    }

    if loader.biomes.is_empty() {
        loader.biomes = biome_names(&registries);
    }
    store.drain_changes(&mut loader.changes);
    if !loader.changes.is_empty() {
        let _adopting = info_span!("stream adopt").entered();
        loader.adopt(store, &definitions, &mut cave);
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
    start_tinting(loader, pool, store);

    let placing = info_span!("stream place").entered();
    let mut meshed = Vec::new();
    loader
        .meshing
        .retain_mut(|(at, slot, task)| match check_ready(task) {
            Some(done) => {
                meshed.push((*at, *slot, done));
                false
            }
            None => true,
        });
    for (at, slot, (mesh, scratch)) in meshed {
        loader.scratches.push(scratch);
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
                        loader.enqueue(victim);
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

    drop(placing);

    let flushing = info_span!("stream flush").entered();
    if loader.dirty
        && let Some(placement) = loader.flush()
    {
        loader.dirty = false;
        loader.uploads.push(Upload::Geometry(placement));
    }
    drop(flushing);

    if !loader.caught_up() {
        return;
    }
    let _admitting = info_span!("stream admit").entered();
    let room = SECTIONS_IN_FLIGHT.saturating_sub(loader.meshing.len());
    let wanted = loader.take_wanted(room.min(SECTIONS_PER_FRAME), store);
    for (taken, &at) in wanted.iter().enumerate() {
        let Some(slot) = loader.take_slot() else {
            for &back in &wanted[taken..] {
                loader.enqueue(back);
            }
            break;
        };
        let world = store.around(ColumnPos::new(at[0], at[2]));
        let blocks = loader.blocks.clone();
        let mut scratch = loader.scratches.pop().unwrap_or_else(Scratch::new);
        loader.pending.insert(at);
        loader.meshing.push((
            at,
            slot,
            pool.spawn(async move {
                let mesh = mesh::mesh_section(&world, &blocks, at, slot, &mut scratch);
                (mesh, scratch)
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
    let sent = loader.sent.clone();
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
                    .enumerate()
                    .map(|(index, array)| {
                        let (first_still, first_frame) = sent.get(index).copied().unwrap_or((0, 0));
                        AtlasUpdate {
                            size: array.size,
                            stills: array.stills() as u32,
                            frames: array.frame_layers() as u32,
                            first_still,
                            first_frame,
                            still_mips: array.still_mips(first_still as usize),
                            frame_mips: array.frame_mips(first_frame as usize),
                        }
                    })
                    .collect(),
                animations: sprites
                    .animations()
                    .iter()
                    .map(|animation| Animation {
                        array: u32::from(animation.array),
                        frame_base: animation.frame_base,
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
        loader.sent = baked.catalog.sprites.counts();
        loader.uploads.push(sprites);
    }
    for failure in &baked.catalog.failures[loader.failures..] {
        warn!("no model for {failure}");
    }
    loader.failures = baked.catalog.failures.len();
    loader.catalog = Some(baked.catalog);
    loader.blocks = Arc::new(baked.blocks);
}

fn start_tinting(loader: &mut Loader, pool: &'static AsyncComputeTaskPool, store: &ColumnStore) {
    let Some(catalog) = loader.catalog.as_ref() else {
        return;
    };
    if catalog.tints.len() <= 1 {
        return;
    }
    let tints = catalog.tints.clone();
    for pos in std::mem::take(&mut loader.to_tint) {
        let corner = loader.tint_corner(pos);
        let world = store.around(pos);
        let tints = tints.clone();
        loader.tinting.push((
            corner,
            pool.spawn(async move { blocks::tint_column(&world, &tints, pos) }),
        ));
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
            connectivity: crate::mesh::OPEN,
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
            flushed.groups[0]
                .1
                .iter()
                .map(|g| g.quad_prefix)
                .collect::<Vec<_>>(),
            [0, 3],
            "the blend order of a bucket runs across the sections in it"
        );

        loader.evict([0, 0, 0], &mut cave);
        let flushed = loader.flush().expect("the group arena has room");
        let draws = flushed
            .draws
            .expect("a flush hands over the whole draw list");
        assert_eq!(draws[0].group_count, 1);
        assert_eq!(draws[0].quad_count, 5);
        assert_eq!(
            flushed.groups[0].1[0].quad_prefix, 0,
            "what the evicted section held is given back, not left as a hole"
        );
    }

    #[test]
    fn a_stream_too_big_to_rebuild_with_room_to_spare_still_draws_every_section() {
        let mut loader = Loader::new(
            &Budget {
                quads: 1 << 12,
                models: 1 << 12,
                faces: 1 << 16,
                groups: 1 << 12,
                sections: 1 << 11,
                tint_size: [512; 2],
            },
            Uploads::default(),
        );
        let mut cave = CaveCull::new(1 << 11);

        // Past a thousand records the roomy ask is the whole arena, which the block still
        // out makes impossible.
        let placed = 1100u32;
        for index in 0..placed {
            loader
                .place(
                    one_greedy_group([index as i32, 0, 0], index, 1),
                    index,
                    &mut cave,
                )
                .unwrap_or_else(|_| panic!("the arena has room"));
            loader
                .flush()
                .expect("a flush hands over the whole draw list");
        }

        let draws = loader
            .flush()
            .expect("a flush hands over the whole draw list")
            .draws
            .expect("the whole draw list");
        assert_eq!(
            draws[0].group_count, placed,
            "every section placed is a record the draw reaches"
        );
        assert_eq!(draws[0].quad_count, placed);
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
        assert!(loader.surrounded(ColumnPos::new(0, 0), &store));
        assert!(
            !loader.surrounded(ColumnPos::new(1, 0), &store),
            "an edge column still has three neighbours missing"
        );

        store.remove(ColumnPos::new(-1, -1));
        assert!(
            !loader.surrounded(ColumnPos::new(0, 0), &store),
            "the diagonal is read too, so losing it is enough"
        );
    }

    #[test]
    fn the_queue_serves_the_nearest_section_first_wherever_the_camera_has_gone() {
        let mut queue = MeshQueue::default();
        let camera = [0, 0, 0];
        for at in [[40, 0, 0], [3, 0, 0], [12, 0, 0], [-7, 0, 0]] {
            queue.insert(at, camera);
        }
        queue.remove([3, 0, 0]);
        assert_eq!(queue.pop_nearest(camera), Some([-7, 0, 0]));
        assert_eq!(
            queue.pop_nearest([36, 0, 0]),
            Some([40, 0, 0]),
            "the camera moved past the step, so the order follows it"
        );
        assert_eq!(queue.pop_nearest([36, 0, 0]), Some([12, 0, 0]));
        assert_eq!(queue.pop_nearest([36, 0, 0]), None);
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn a_section_is_queued_when_the_last_of_its_neighbours_arrives_not_when_it_does() {
        use mcrs_minecraft_network::columns::{Column, Extent, Section};

        let extent = Extent {
            min_section_y: 0,
            sections: 1,
        };
        let stone = || {
            Column::unlit(
                0,
                vec![Some(Section {
                    blocks: Box::new([1; mcrs_minecraft_network::columns::SECTION_VOLUME]),
                    biomes: Box::new([0; mcrs_minecraft_network::columns::BIOME_CELLS]),
                    states: vec![1],
                })],
            )
        };

        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8);
        let mut store = ColumnStore::default();
        store.enter(extent);

        let mut order: Vec<ColumnPos> = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| ColumnPos::new(x, z)))
            .collect();
        let last = order.pop().expect("nine columns");
        for pos in &order {
            store.insert(*pos, stone());
        }
        store.drain_changes(&mut loader.changes);
        loader.adopt(&store, blocks::corpus(), &mut cave);
        assert!(
            loader.queue.is_empty(),
            "no column is surrounded yet, so nothing is meshable"
        );

        store.insert(last, stone());
        store.drain_changes(&mut loader.changes);
        loader.adopt(&store, blocks::corpus(), &mut cave);
        assert!(
            loader.queue.contains(&[0, 0, 0]),
            "the centre column arrived first; only its last neighbour makes it meshable"
        );
        assert_ne!(
            last,
            ColumnPos::new(0, 0),
            "the arrival that unblocked the centre was a neighbour, not the centre itself"
        );

        assert_eq!(loader.take_wanted(8, &store), vec![[0, 0, 0]]);
        assert!(
            loader.take_wanted(8, &store).is_empty(),
            "a section handed out once is not handed out again"
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
    fn a_column_wraps_into_the_tint_window_wherever_it_is() {
        let loader = loader();
        assert_eq!(loader.tint_corner(ColumnPos::new(0, 0)), [0, 0]);
        assert_eq!(loader.tint_corner(ColumnPos::new(31, 1)), [496, 16]);
        assert_eq!(loader.tint_corner(ColumnPos::new(32, 0)), [0, 0]);
        assert_eq!(loader.tint_corner(ColumnPos::new(-1, -33)), [496, 496]);
        assert_eq!(loader.tint_corner(ColumnPos::new(1_000_000, 0)), [0, 0]);
    }
}
