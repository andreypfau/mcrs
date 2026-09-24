use std::sync::{Arc, LazyLock};

use crate::columns::{
    BlockSource, ClientTerrainSet, ColumnChange, ColumnStore, Extent, SECTION_SIZE,
};
use bevy::ecs::system::SystemParam;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures::check_ready};
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_level::world::lifecycle::trace::{ColumnStage, ColumnTraceSink, TraceEvent};
use mcrs_minecraft_network::client::{ClientConnection, JoinedGame, ReceivedRegistries};

use crate::atlas::SpriteArray;
use crate::blocks::{self, Catalog};
use crate::cave::{CaveCull, NO_SLOT};
use crate::item_model::bake::{ItemModels, bake_all as bake_items};
use crate::model::Pack;
use crate::render::{
    Animation, AtlasUpdate, Budget, FACE_BYTES, Placement, STILL, SectionDesc, SpriteEntry,
    SpriteUpload, Upload, Uploads,
};
use crate::vanilla::VanillaAssets;
use mcrs_minecraft_mesh::arena::{Arena, Block};
use mcrs_minecraft_mesh::block::BlockInfo;
use mcrs_minecraft_mesh::pack::QUAD_WORDS;
use mcrs_minecraft_mesh::{self as mesh, Connectivity, Draw, Group, STREAMS, Scratch, SectionMesh};

const HYSTERESIS: f32 = (16 * SECTION_SIZE) as f32;

static SECTIONS_IN_FLIGHT: LazyLock<usize> = LazyLock::new(crate::config::mesh_in_flight);

static SECTIONS_PER_FRAME: LazyLock<usize> = LazyLock::new(crate::config::mesh_per_frame);

const BIOME_REGISTRY: &str = "minecraft:worldgen/biome";

pub struct StreamPlugin {
    budget: Arc<Budget>,
    uploads: Uploads,
}

impl StreamPlugin {
    pub fn new(budget: Arc<Budget>, uploads: Uploads) -> Self {
        Self { budget, uploads }
    }
}

impl Plugin for StreamPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Loader::new(&self.budget))
            .insert_resource(BlockCatalog::new())
            .insert_resource(ColumnTints::new(self.budget.tint_size))
            .insert_resource(self.uploads.clone())
            .add_systems(
                Update,
                (
                    follow_camera,
                    adopt_columns,
                    bake_catalog.run_if(in_state(VanillaAssets::Ready)),
                    tint_columns,
                    place_meshes,
                    flush_streams,
                    record_traces,
                    admit_meshing,
                )
                    .chain()
                    .in_set(ClientTerrainSet::Build)
                    .run_if(can_stream),
            );
    }
}

/// The sections the server's columns hold, from queued to drawn, and the arena they are
/// drawn out of.
#[derive(Resource)]
pub struct Loader {
    changes: Vec<ColumnChange>,
    columns: usize,
    sections_total: usize,
    sections: Sections,
    /// Sections whose light a delta rewrote, present or not.
    relit: HashSet<[i32; 3]>,
    meshing: Vec<([i32; 3], u32, Task<(SectionMesh, Scratch)>)>,
    scratches: Vec<Scratch>,
    streams: [Stream; STREAMS],
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
    trace: Vec<TraceEvent>,
}

/// The block states the catalog has baked and still owes, and the bake running on the pool.
#[derive(Resource)]
pub struct BlockCatalog {
    pack: PackLoad,
    catalog: Option<Catalog>,
    blocks: Arc<Vec<BlockInfo>>,
    baked: Vec<bool>,
    to_bake: Vec<u16>,
    baking: Option<Task<Baked>>,
    foreign_states: bool,
    failures: usize,
    biomes: Vec<String>,
    sprites: usize,
    sent: Vec<u32>,
    items_baked: bool,
}

#[derive(Resource)]
pub struct ColumnTints {
    to_tint: Vec<ColumnPos>,
    tinting: Vec<([u32; 2], Task<Vec<u8>>)>,
    size: [u32; 2],
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

enum SectionState {
    Queued,
    /// On the compute pool. An orphaned mesh was read out of a column that left or was sent
    /// again meanwhile, so the world it holds is one nobody holds any more.
    Meshing {
        orphaned: bool,
    },
    Resident(Resident),
    /// Meshed with no room in the arena, until an eviction or the camera moving makes some.
    Deferred,
}

/// One stream's group records as the GPU holds them, dead ones kept as records with no quads
/// until the stream is rebuilt, so a change only ever writes the records it touched.
struct Stream {
    groups: Vec<Group>,
    block: Block,
    quads_end: u32,
    dead: usize,
    touched: Vec<(u32, u32)>,
    rebuild: bool,
    /// The arena had no room for this stream even with every stream packed tight, so it stops
    /// asking until a rebuild elsewhere moves room around; retrying every frame would lay out
    /// and send every stream again for nothing.
    starved: bool,
}

struct Baked {
    catalog: Catalog,
    blocks: Vec<BlockInfo>,
    sprites: Option<Upload>,
    items: Option<ItemModels>,
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

#[derive(SystemParam)]
pub struct Streaming<'w> {
    loader: Res<'w, Loader>,
    catalog: Res<'w, BlockCatalog>,
    tints: Res<'w, ColumnTints>,
    uploads: Res<'w, Uploads>,
}

impl Streaming<'_> {
    pub fn status(&self) -> Status {
        let loader = &self.loader;
        Status {
            columns: loader.columns,
            sections: loader.sections.tally.residents,
            sections_total: loader.sections_total,
            evicted: loader.evicted,
            quads: loader.quads.held() as f32 / loader.quads.capacity() as f32,
            models: loader.models.held() as f32 / loader.models.capacity() as f32,
            faces: loader.faces.held() as f32 / loader.faces.capacity() as f32,
            groups: loader.groups.held() as f32 / loader.groups.capacity() as f32,
            queued: loader.sections.queued(),
            meshing: loader.meshing.len(),
            uploads_waiting: self.uploads.waiting(),
        }
    }

    pub fn done(&self) -> bool {
        self.idle() && self.uploads.waiting() == 0
    }

    fn idle(&self) -> bool {
        self.catalog.to_bake.is_empty()
            && self.catalog.baking.is_none()
            && self.tints.to_tint.is_empty()
            && self.tints.tinting.is_empty()
            && self.loader.meshing.is_empty()
            && !self.loader.dirty
            && self.loader.sections.queued() == 0
    }
}

impl Loader {
    pub fn new(budget: &Budget) -> Self {
        Self {
            changes: Vec::new(),
            columns: 0,
            sections_total: 0,
            sections: Sections::default(),
            relit: HashSet::new(),
            meshing: Vec::new(),
            scratches: Vec::new(),
            streams: std::array::from_fn(|_| Stream::new()),
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
            trace: Vec::new(),
        }
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
        !matches!(
            self.sections.state(at),
            Some(SectionState::Resident(_) | SectionState::Meshing { .. })
        ) && store.section(at[0], at[1], at[2]).is_some()
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
        self.sections.enqueue(at, camera);
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

    /// A section whose light a delta rewrote has to be meshed again, and so do the
    /// six around it: a face samples the light of the cell it faces, which for a
    /// face on the section boundary lies in the neighbour. One still being meshed
    /// waits for the next frame, since the task holds the light it started with.
    fn remesh_relit(&mut self, store: &ColumnStore, cave: &mut CaveCull) {
        for at in std::mem::take(&mut self.relit) {
            if self.sections.is_meshing(at) {
                self.relit.insert(at);
                continue;
            }
            self.evict(at, cave);
            if self.meshable(at, store) {
                self.enqueue(at);
            }
        }
    }

    /// Takes a landed mesh's section off the pool. A mesh read out of a column the server has
    /// since taken back is thrown away rather than drawn, and its section queued again if the
    /// column came back while the task ran.
    fn discard_orphan(&mut self, at: [i32; 3], slot: u32, store: &ColumnStore) -> bool {
        if self.sections.finish_meshing(at) != Some(true) {
            return false;
        }
        self.free_slots.push(slot);
        if self.meshable(at, store) {
            self.enqueue(at);
        }
        true
    }

    fn requeue_deferred(&mut self) {
        let camera = self.camera_section();
        self.sections.requeue_deferred(camera);
    }

    /// The nearest queued sections. Nothing more than distance decides the order yet.
    fn take_wanted(&mut self, want: usize, store: &ColumnStore) -> Vec<[i32; 3]> {
        if want == 0 || self.sections.queued() == 0 {
            return Vec::new();
        }
        let camera = self.camera_section();
        let mut taken = Vec::with_capacity(want);
        while taken.len() < want {
            let Some(at) = self.sections.pop_nearest(camera) else {
                break;
            };
            if self.meshable(at, store) {
                taken.push(at);
            }
        }
        taken
    }

    fn victim(&self, candidate: f32) -> Option<[i32; 3]> {
        let farthest = self
            .sections
            .residents()
            .map(|(at, _)| at)
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

    fn evict(&mut self, section: [i32; 3], cave: &mut CaveCull) {
        let Some(held) = self.sections.take_resident(section) else {
            return;
        };
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.faces.free(held.faces);
        if held.slot != NO_SLOT {
            for (stream, &(first, count)) in self.streams.iter_mut().zip(&held.groups) {
                if count == 0 {
                    continue;
                }
                let (first, end) = (first as usize, (first + count) as usize);
                for group in &mut stream.groups[first..end] {
                    group.quad_count = 0;
                }
                stream.dead += count as usize;
                stream.touched.push((first as u32, end as u32));
                if stream.dead * 2 >= stream.groups.len() {
                    stream.rebuild = true;
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
        for (at, held) in self.sections.residents() {
            cave.set_section(at, held.slot, held.connectivity);
        }
    }

    /// Takes what the server sent and took back since the last frame: a departed column's
    /// sections leave the queue and the arena with it, and an arrived column is walked for the
    /// block states the catalog still owes and queued for its tints.
    fn adopt(
        &mut self,
        store: &ColumnStore,
        definitions: &BlockDefinitions,
        catalog: &mut BlockCatalog,
        tints: &mut ColumnTints,
        cave: &mut CaveCull,
    ) {
        let extent = store.extent();
        catalog.cover(definitions);
        let mut changes = std::mem::take(&mut self.changes);
        for change in changes.drain(..) {
            match change {
                ColumnChange::Departed(pos, column) => {
                    self.trace.push(TraceEvent::Forget(pos));
                    self.columns -= 1;
                    tints.to_tint.retain(|queued| *queued != pos);
                    for (sy, section) in column.sections() {
                        if section.is_some() {
                            self.sections_total -= 1;
                        }
                        let at = [pos.x, sy, pos.z];
                        self.sections.depart(at);
                        self.evict(at, cave);
                    }
                }
                ColumnChange::Arrived(pos, column) => {
                    self.trace
                        .push(TraceEvent::mark(pos, ColumnStage::Received));
                    self.columns += 1;
                    tints.to_tint.push(pos);
                    // The column the change carries, not the one the store holds now: a column
                    // the server took back before this drain is already gone from the store,
                    // and its departure counts the sections this arrival has to have counted.
                    for (_, section) in column.sections() {
                        let Some(section) = section else { continue };
                        self.sections_total += 1;
                        for &state in &section.states {
                            catalog.owe(state);
                        }
                    }
                    if let Some(extent) = extent {
                        self.enqueue_around(pos, extent, store);
                    }
                }
                ColumnChange::Relit(pos, rows) => {
                    // A vertex takes its shade from the four cells around it, so
                    // a section's mesh depends on the sections diagonally past
                    // its edges as much as on the ones across its faces. Leaving
                    // the diagonals stale draws a dark line along the seam.
                    for sy in rows {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                for dx in -1..=1 {
                                    self.relit.insert([pos.x + dx, sy + dy, pos.z + dz]);
                                }
                            }
                        }
                    }
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
        for (index, stream) in self.streams.iter_mut().enumerate() {
            let run = mesh.spans[index].group_count as usize;
            let base = if index % 2 == 0 {
                quads.offset
            } else {
                models.offset
            } as u32;
            for group in &mut placed[first..first + run] {
                group.quad_base += base;
                group.quad_prefix = stream.quads_end;
                stream.quads_end += group.quad_count;
            }
            if run != 0 {
                let at = stream.groups.len() as u32;
                stream.groups.extend_from_slice(&placed[first..first + run]);
                ranges[index] = (at, run as u32);
                stream.touched.push((at, at + run as u32));
                if stream.groups.len() > stream.block.capacity() {
                    stream.rebuild = true;
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
        self.trace.push(TraceEvent::mark(
            ColumnPos::new(section[0], section[2]),
            ColumnStage::Meshed,
        ));
        self.sections.settle(
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
            faces: ((faces.offset * FACE_BYTES) as u64, mesh.faces),
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

    /// Writes the records each stream touched since the last flush, after any stream that
    /// outgrew its block or filled it with dead records has been given a new one. What lies
    /// past a stream's block is left for the rebuild that reaches it.
    fn flush(&mut self) -> Option<Placement> {
        self.rebuild_blocks();
        let mut groups = Vec::new();
        for stream in &mut self.streams {
            let capacity = stream.block.capacity() as u32;
            let mut ranges = std::mem::take(&mut stream.touched);
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
            if stream.rebuild {
                // What lies past the block is written once the rebuild goes through.
                stream.touched.push((capacity, stream.groups.len() as u32));
            }
            for (first, end) in merged {
                let offset = (stream.block.offset + first as usize) * size_of::<Group>();
                groups.push((
                    offset as u64,
                    stream.groups[first as usize..end as usize].to_vec(),
                ));
            }
        }
        let draws = self
            .streams
            .iter()
            .enumerate()
            .map(|(index, stream)| Draw {
                stream: index as u32,
                first_group: stream.block.offset as u32,
                group_count: stream.groups.len().min(stream.block.capacity()) as u32,
                quad_count: stream.quads_end,
            })
            .collect();
        Some(Placement {
            groups,
            draws: Some(draws),
            ..Placement::default()
        })
    }

    /// Gives a fresh block to every stream that outgrew the one it has or filled it with dead
    /// records. The block a stream leaves is room for the one it takes, since every record is
    /// written again; when what the arena has left is enough but not in one piece, every stream
    /// is laid out again to put it back together.
    fn rebuild_blocks(&mut self) {
        let mut in_pieces = false;
        for index in 0..STREAMS {
            let stream = &mut self.streams[index];
            if !stream.rebuild || stream.starved {
                continue;
            }
            let stale = std::mem::replace(&mut stream.block, Block::EMPTY);
            self.groups.free(stale);
            match self.groups.alloc(stream.live().max(MIN_BLOCK)) {
                Some(block) => {
                    stream.block = block;
                    // A rebuild that goes through moves room around, so a stream that was
                    // refused can ask again.
                    for stream in &mut self.streams {
                        stream.starved = false;
                    }
                    self.pack_stream(index);
                }
                None => in_pieces = true,
            }
        }
        if in_pieces {
            self.repack_streams();
        }
    }

    /// Lays every stream out again, the largest first so the blocks stack without leaving a
    /// hole a later one cannot use. A stream the arena cannot hold even packed tight takes the
    /// largest block it can, since one that takes none draws nothing at all.
    fn repack_streams(&mut self) {
        for stream in &mut self.streams {
            let stale = std::mem::replace(&mut stream.block, Block::EMPTY);
            self.groups.free(stale);
        }
        let mut order: [usize; STREAMS] = std::array::from_fn(|index| index);
        order.sort_unstable_by_key(|&index| std::cmp::Reverse(self.streams[index].live()));
        for index in order {
            let stream = &mut self.streams[index];
            let live = stream.live();
            let mut want = live.max(MIN_BLOCK).next_power_of_two();
            while stream.block.capacity() == 0 {
                match self.groups.alloc(want) {
                    Some(block) => stream.block = block,
                    None if want > MIN_BLOCK => want /= 2,
                    None => break,
                }
            }
            stream.starved = stream.block.capacity() < live;
            if stream.starved {
                warn!(
                    stream = index,
                    live,
                    held = self.groups.held(),
                    capacity = self.groups.capacity(),
                    "the group arena cannot hold every stream packed tight; this one draws \
                     only what its block holds and the rest of its sections stay off the screen"
                );
            }
            self.pack_stream(index);
        }
    }

    /// Packs a stream's live records into the block it holds, keeping their order and telling
    /// each resident section where its records went.
    fn pack_stream(&mut self, index: usize) {
        let Self {
            streams,
            owners,
            sections,
            ..
        } = self;
        let stream = &mut streams[index];
        let live = stream.live();
        let old = std::mem::take(&mut stream.groups);
        let mut packed = Vec::with_capacity(live);
        let mut quads = 0u32;
        let mut last_slot = NO_SLOT;
        for mut group in old.into_iter().filter(|group| group.quad_count != 0) {
            let owner = owners[group.section as usize];
            if let Some(resident) = sections.resident_mut(owner) {
                if group.section != last_slot {
                    resident.groups[index] = (packed.len() as u32, 0);
                }
                resident.groups[index].1 += 1;
            }
            last_slot = group.section;
            group.quad_prefix = quads;
            quads += group.quad_count;
            packed.push(group);
        }
        stream.groups = packed;
        stream.quads_end = quads;
        stream.dead = 0;
        stream.touched.clear();
        stream.touched.push((0, live as u32));
        stream.rebuild = false;
    }
}

impl Stream {
    fn new() -> Self {
        Self {
            groups: Vec::new(),
            block: Block::EMPTY,
            quads_end: 0,
            dead: 0,
            touched: Vec::new(),
            rebuild: false,
            starved: false,
        }
    }

    /// The records the stream still draws, the dead ones it has not been rebuilt out of aside.
    fn live(&self) -> usize {
        self.groups.len() - self.dead
    }
}

impl BlockCatalog {
    pub fn pack(&self) -> Option<&Arc<Pack>> {
        match &self.pack {
            PackLoad::Ready(pack) => Some(pack),
            _ => None,
        }
    }

    fn new() -> Self {
        Self {
            pack: PackLoad::Pending,
            catalog: Some(blocks::empty()),
            blocks: Arc::new(Vec::new()),
            baked: Vec::new(),
            to_bake: Vec::new(),
            baking: None,
            foreign_states: false,
            failures: 0,
            biomes: Vec::new(),
            sprites: 0,
            sent: Vec::new(),
            items_baked: false,
        }
    }

    /// The catalog has to reach every state a resident column names before a
    /// section holding one can be meshed against it.
    fn caught_up(&self) -> bool {
        self.to_bake.is_empty() && self.baking.is_none() && !self.blocks.is_empty()
    }

    fn cover(&mut self, definitions: &BlockDefinitions) {
        if self.baked.len() < definitions.state_count() {
            self.baked.resize(definitions.state_count(), false);
        }
    }

    fn owe(&mut self, state: u16) {
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

    /// The pack is read once, off the asset system, before anything can bake against it.
    fn poll_pack(&mut self, assets: &AssetServer) -> Option<Arc<Pack>> {
        match &mut self.pack {
            PackLoad::Pending => {
                let assets = assets.clone();
                self.pack = PackLoad::Loading(
                    IoTaskPool::get().spawn(async move { Pack::load(&assets).await }),
                );
                None
            }
            PackLoad::Loading(task) => {
                let pack = check_ready(task)?
                    .unwrap_or_else(|reason| panic!("cannot read the resource pack: {reason}"));
                info!(files = pack.len(), "read the resource pack");
                let pack = Arc::new(pack);
                self.pack = PackLoad::Ready(pack.clone());
                Some(pack)
            }
            PackLoad::Ready(pack) => Some(pack.clone()),
        }
    }

    fn start_baking(
        &mut self,
        pack: &Arc<Pack>,
        definitions: &Blocks,
        pool: &'static AsyncComputeTaskPool,
    ) {
        let Some(mut catalog) = self.catalog.take() else {
            return;
        };
        let states = std::mem::take(&mut self.to_bake);
        let biomes = self.biomes.clone();
        let known = self.sprites;
        let sent = self.sent.clone();
        let definitions = definitions.clone();
        let pack = pack.clone();
        let bake_items_too = !self.items_baked;
        self.items_baked = true;
        self.baking = Some(pool.spawn(async move {
            blocks::extend(&pack, &mut catalog, &definitions, &states, &biomes);
            let items = bake_items_too.then(|| {
                let started = std::time::Instant::now();
                let items = bake_items(&pack, &mut catalog.sprites)
                    .unwrap_or_else(|reason| panic!("cannot bake the item models: {reason}"));
                info!(items = items.by_id.len(), elapsed = ?started.elapsed(), "baked the item models");
                items
            });
            let blocks = catalog.blocks.clone();
            let sprites = (catalog.sprites.len() != known).then(|| {
                let sprites = &catalog.sprites;
                Upload::Sprites(SpriteUpload {
                    atlases: sprites
                        .arrays()
                        .iter()
                        .enumerate()
                        .map(|(index, array)| {
                            let first = sent.get(index).copied().unwrap_or(0);
                            AtlasUpdate {
                                size: array.size,
                                layers: array.layers(),
                                first,
                                mips: array.mips(first as usize),
                            }
                        })
                        .collect(),
                    table_from: known as u32,
                    table: sprites.table()[known..]
                        .iter()
                        .map(|sprite| SpriteEntry {
                            array_layer: u32::from(sprite.array) << 16 | u32::from(sprite.layer),
                            animation: sprite.animation.map_or(STILL, u32::from),
                        })
                        .collect(),
                    animations: sprites
                        .animations()
                        .iter()
                        .map(|animation| Animation {
                            first_layer: u32::from(animation.first_layer),
                            count: animation.count,
                            frametime: animation.frametime,
                            interpolate: u32::from(animation.interpolate),
                        })
                        .collect(),
                })
            });
            Baked {
                catalog,
                blocks,
                sprites,
                items,
            }
        }));
    }

    fn publish(&mut self, baked: Baked, uploads: &Uploads) -> Option<ItemModels> {
        if let Some(sprites) = baked.sprites {
            self.sprites = baked.catalog.sprites.len();
            self.sent = baked
                .catalog
                .sprites
                .arrays()
                .iter()
                .map(SpriteArray::layers)
                .collect();
            uploads.push(sprites);
        }
        for failure in &baked.catalog.failures[self.failures..] {
            warn!("no model for {failure}");
        }
        self.failures = baked.catalog.failures.len();
        self.catalog = Some(baked.catalog);
        self.blocks = Arc::new(baked.blocks);
        baked.items
    }
}

impl ColumnTints {
    pub fn new(size: [u32; 2]) -> Self {
        Self {
            to_tint: Vec::new(),
            tinting: Vec::new(),
            size,
        }
    }

    /// Where a column's tint square sits in the tint texture, which the world wraps into.
    fn corner(&self, pos: ColumnPos) -> [u32; 2] {
        let wrapped = |axis: i32, size: u32| (axis * SECTION_SIZE as i32).rem_euclid(size as i32);
        [
            wrapped(pos.x, self.size[0]) as u32,
            wrapped(pos.z, self.size[1]) as u32,
        ]
    }
}

/// Touched runs of records this close together are written as one copy.
const RUN_GAP: u32 = 64;
/// The smallest block a stream is rebuilt into, in records.
const MIN_BLOCK: usize = 256;

/// Every section the loader knows of by what it is doing, with the queue's buckets and the
/// counts the status reads kept beside the states by the only code that changes them.
#[derive(Default)]
struct Sections {
    states: HashMap<[i32; 3], SectionState>,
    queue: MeshQueue,
    tally: Tally,
}

#[derive(Default)]
struct Tally {
    queued: usize,
    residents: usize,
    /// Every eviction requeues these, so they are not found by walking every state.
    deferred: HashSet<[i32; 3]>,
}

impl Tally {
    fn enter(&mut self, at: [i32; 3], state: &SectionState) {
        match state {
            SectionState::Queued => self.queued += 1,
            SectionState::Resident(_) => self.residents += 1,
            SectionState::Deferred => {
                self.deferred.insert(at);
            }
            SectionState::Meshing { .. } => {}
        }
    }

    fn leave(&mut self, at: [i32; 3], state: &SectionState) {
        match state {
            SectionState::Queued => self.queued -= 1,
            SectionState::Resident(_) => self.residents -= 1,
            SectionState::Deferred => {
                self.deferred.remove(&at);
            }
            SectionState::Meshing { .. } => {}
        }
    }
}

impl Sections {
    fn state(&self, at: [i32; 3]) -> Option<&SectionState> {
        self.states.get(&at)
    }

    fn replace(&mut self, at: [i32; 3], next: Option<SectionState>) -> Option<SectionState> {
        if let Some(next) = &next {
            self.tally.enter(at, next);
        }
        let previous = match next {
            Some(next) => self.states.insert(at, next),
            None => self.states.remove(&at),
        };
        if let Some(previous) = &previous {
            self.tally.leave(at, previous);
        }
        previous
    }

    fn queued(&self) -> usize {
        self.tally.queued
    }

    #[cfg(test)]
    fn is_queued(&self, at: [i32; 3]) -> bool {
        matches!(self.state(at), Some(SectionState::Queued))
    }

    fn is_meshing(&self, at: [i32; 3]) -> bool {
        matches!(self.state(at), Some(SectionState::Meshing { .. }))
    }

    #[cfg(test)]
    fn is_resident(&self, at: [i32; 3]) -> bool {
        matches!(self.state(at), Some(SectionState::Resident(_)))
    }

    fn residents(&self) -> impl Iterator<Item = ([i32; 3], &Resident)> {
        self.states.iter().filter_map(|(at, state)| match state {
            SectionState::Resident(held) => Some((*at, held)),
            _ => None,
        })
    }

    fn resident_mut(&mut self, at: [i32; 3]) -> Option<&mut Resident> {
        match self.states.get_mut(&at) {
            Some(SectionState::Resident(held)) => Some(held),
            _ => None,
        }
    }

    /// Only a section nothing holds, or one waiting for room, can be queued.
    fn enqueue(&mut self, at: [i32; 3], camera: [i32; 3]) {
        if !matches!(self.state(at), None | Some(SectionState::Deferred)) {
            return;
        }
        self.replace(at, Some(SectionState::Queued));
        self.queue.push(at, camera);
    }

    #[cfg(test)]
    fn dequeue(&mut self, at: [i32; 3]) {
        if matches!(self.state(at), Some(SectionState::Queued)) {
            self.replace(at, None);
        }
    }

    /// The queued section nearest the camera, taken out of the queue.
    fn pop_nearest(&mut self, camera: [i32; 3]) -> Option<[i32; 3]> {
        if self.tally.queued == 0 {
            return None;
        }
        let Self {
            states,
            queue,
            tally,
        } = self;
        let at = queue.pop_nearest(camera, tally.queued, |at| {
            matches!(states.get(&at), Some(SectionState::Queued))
        })?;
        self.replace(at, None);
        Some(at)
    }

    #[cfg(test)]
    fn rebucket(&mut self, camera: [i32; 3]) {
        let Self { states, queue, .. } = self;
        queue.rebucket(camera, |at| {
            matches!(states.get(&at), Some(SectionState::Queued))
        });
    }

    fn start_meshing(&mut self, at: [i32; 3]) {
        self.replace(at, Some(SectionState::Meshing { orphaned: false }));
    }

    /// Ends a section's meshing, telling whether the mesh was orphaned on the way.
    fn finish_meshing(&mut self, at: [i32; 3]) -> Option<bool> {
        match self.state(at) {
            Some(SectionState::Meshing { orphaned }) => {
                let orphaned = *orphaned;
                self.replace(at, None);
                Some(orphaned)
            }
            _ => None,
        }
    }

    fn settle(&mut self, at: [i32; 3], held: Resident) {
        self.replace(at, Some(SectionState::Resident(held)));
    }

    fn defer(&mut self, at: [i32; 3]) {
        self.replace(at, Some(SectionState::Deferred));
    }

    fn take_resident(&mut self, at: [i32; 3]) -> Option<Resident> {
        if !matches!(self.state(at), Some(SectionState::Resident(_))) {
            return None;
        }
        match self.replace(at, None) {
            Some(SectionState::Resident(held)) => Some(held),
            _ => None,
        }
    }

    /// The section's column left: it stops waiting, and a mesh still being read out of it is
    /// orphaned. A resident section is the caller's to evict.
    fn depart(&mut self, at: [i32; 3]) {
        match self.states.get_mut(&at) {
            Some(SectionState::Meshing { orphaned }) => *orphaned = true,
            Some(SectionState::Queued | SectionState::Deferred) => {
                self.replace(at, None);
            }
            Some(SectionState::Resident(_)) | None => {}
        }
    }

    fn requeue_deferred(&mut self, camera: [i32; 3]) {
        for at in std::mem::take(&mut self.tally.deferred) {
            self.enqueue(at, camera);
        }
    }
}

/// Sections waiting to be meshed, bucketed by how many sections they lie from a reference
/// position along their farthest axis, so the nearest come out first without ever sorting the
/// whole queue. The reference follows the camera in steps of `QUEUE_STEP` sections, and each
/// step rebuckets everything, so the order is never staler than that. Whether an entry is
/// still queued is the section's state, not the bucket's.
#[derive(Default)]
struct MeshQueue {
    buckets: Vec<Vec<[i32; 3]>>,
    reference: [i32; 3],
    /// Entries across the buckets, those since taken out of the queue included.
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

    fn push(&mut self, at: [i32; 3], camera: [i32; 3]) {
        if self.buckets.is_empty() {
            self.buckets = (0..QUEUE_BUCKETS).map(|_| Vec::new()).collect();
            self.reference = camera;
        }
        self.buckets[Self::bucket(at, self.reference)].push(at);
        self.entries += 1;
    }

    fn pop_nearest(
        &mut self,
        camera: [i32; 3],
        queued: usize,
        is_queued: impl Fn([i32; 3]) -> bool,
    ) -> Option<[i32; 3]> {
        let moved = Self::bucket(camera, self.reference) >= QUEUE_STEP;
        if moved || self.entries > 2 * queued + 1024 {
            self.rebucket(camera, &is_queued);
        }
        for bucket in &mut self.buckets {
            while let Some(at) = bucket.pop() {
                self.entries -= 1;
                if is_queued(at) {
                    return Some(at);
                }
            }
        }
        None
    }

    /// Buckets every live entry afresh around the camera, dropping the entries whose sections
    /// have since left the queue.
    fn rebucket(&mut self, camera: [i32; 3], is_queued: impl Fn([i32; 3]) -> bool) {
        self.scratch.clear();
        for bucket in &mut self.buckets {
            self.scratch
                .extend(bucket.drain(..).filter(|at| is_queued(*at)));
        }
        self.reference = camera;
        self.entries = self.scratch.len();
        for at in self.scratch.drain(..) {
            self.buckets[Self::bucket(at, camera)].push(at);
        }
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

fn can_stream(store: Option<Res<ColumnStore>>, cameras: Query<(), With<Camera3d>>) -> bool {
    store.is_some() && cameras.single().is_ok()
}

fn follow_camera(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let loader = &mut *loader;
    loader.camera = camera.translation();
    loader.follow(&mut cave);
    if loader.camera.distance(loader.anchor) > HYSTERESIS {
        loader.anchor = loader.camera;
        loader.requeue_deferred();
    }
}

fn adopt_columns(
    mut loader: ResMut<Loader>,
    mut catalog: ResMut<BlockCatalog>,
    mut tints: ResMut<ColumnTints>,
    mut cave: ResMut<CaveCull>,
    mut store: ResMut<ColumnStore>,
    definitions: Res<Blocks>,
) {
    let loader = &mut *loader;
    let store = store.bypass_change_detection();
    store.drain_changes(&mut loader.changes);
    if loader.changes.is_empty() {
        return;
    }
    let _adopting = info_span!("stream adopt").entered();
    loader.adopt(store, &definitions, &mut catalog, &mut tints, &mut cave);
}

fn bake_catalog(
    mut catalog: ResMut<BlockCatalog>,
    uploads: Res<Uploads>,
    assets: Res<AssetServer>,
    definitions: Res<Blocks>,
    registries: Query<&ReceivedRegistries>,
    mut commands: Commands,
) {
    let catalog = &mut *catalog;
    let pack = catalog.poll_pack(&assets);
    if catalog.biomes.is_empty() {
        catalog.biomes = biome_names(&registries);
    }
    if let Some(task) = catalog.baking.as_mut()
        && let Some(baked) = check_ready(task)
    {
        catalog.baking = None;
        if let Some(items) = catalog.publish(baked, &uploads) {
            commands.insert_resource(items);
        }
    }
    if let Some(pack) = pack
        && catalog.baking.is_none()
        && (!catalog.to_bake.is_empty() || !catalog.items_baked)
        && !catalog.biomes.is_empty()
    {
        catalog.start_baking(&pack, &definitions, AsyncComputeTaskPool::get());
    }
}

fn tint_columns(
    mut tints: ResMut<ColumnTints>,
    catalog: Res<BlockCatalog>,
    uploads: Res<Uploads>,
    store: Res<ColumnStore>,
) {
    let tints = &mut *tints;
    tints
        .tinting
        .retain_mut(|(origin, task)| match check_ready(task) {
            Some(data) => {
                uploads.push(Upload::Tints {
                    origin: *origin,
                    size: SECTION_SIZE as u32,
                    data,
                });
                false
            }
            None => true,
        });
    let Some(catalog) = catalog.catalog.as_ref() else {
        return;
    };
    if catalog.tints.len() <= 1 {
        return;
    }
    let pool = AsyncComputeTaskPool::get();
    let palette = catalog.tints.clone();
    for pos in std::mem::take(&mut tints.to_tint) {
        let corner = tints.corner(pos);
        let world = store.around(pos);
        let palette = palette.clone();
        tints.tinting.push((
            corner,
            pool.spawn(async move { blocks::tint_column(&world, &palette, pos) }),
        ));
    }
}

/// A section's mesh off the pool, with the slot it was meshed for and the scratch it borrowed.
type Landed = ([i32; 3], u32, (SectionMesh, Scratch));

fn place_meshes(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
    uploads: Res<Uploads>,
    store: Res<ColumnStore>,
    mut meshed: Local<Vec<Landed>>,
) {
    let _placing = info_span!("stream place").entered();
    let loader = &mut *loader;
    loader
        .meshing
        .retain_mut(|(at, slot, task)| match check_ready(task) {
            Some(done) => {
                meshed.push((*at, *slot, done));
                false
            }
            None => true,
        });
    for (at, slot, (mesh, scratch)) in meshed.drain(..) {
        loader.scratches.push(scratch);
        if loader.discard_orphan(at, slot, &store) {
            continue;
        }
        let here = loader.distance(at);
        let mut pending = mesh;
        loop {
            match loader.place(pending, slot, &mut cave) {
                Ok(placement) => {
                    if !placement.sections.1.is_empty() {
                        uploads.push(Upload::Geometry(placement));
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
                        loader.sections.defer(at);
                        loader.free_slots.push(slot);
                        break;
                    }
                },
            }
        }
    }

    loader.remesh_relit(&store, &mut cave);
}

fn flush_streams(mut loader: ResMut<Loader>, uploads: Res<Uploads>) {
    let _flushing = info_span!("stream flush").entered();
    if loader.dirty
        && let Some(placement) = loader.flush()
    {
        loader.dirty = false;
        uploads.push(Upload::Geometry(placement));
    }
}

fn record_traces(
    mut loader: ResMut<Loader>,
    traces: Option<Res<ColumnTraceSink>>,
    joined: Option<Single<&JoinedGame, With<ClientConnection>>>,
) {
    match (&traces, &joined) {
        (Some(traces), Some(joined)) => traces.record(&joined.dimension, loader.trace.drain(..)),
        _ => loader.trace.clear(),
    }
}

fn admit_meshing(mut loader: ResMut<Loader>, catalog: Res<BlockCatalog>, store: Res<ColumnStore>) {
    if !catalog.caught_up() {
        return;
    }
    let _admitting = info_span!("stream admit").entered();
    let loader = &mut *loader;
    let pool = AsyncComputeTaskPool::get();
    let room = SECTIONS_IN_FLIGHT.saturating_sub(loader.meshing.len());
    let wanted = loader.take_wanted(room.min(*SECTIONS_PER_FRAME), &store);
    for (taken, &at) in wanted.iter().enumerate() {
        let Some(slot) = loader.take_slot() else {
            for &back in &wanted[taken..] {
                loader.enqueue(back);
            }
            break;
        };
        let world = store.around(ColumnPos::new(at[0], at[2]));
        let blocks = catalog.blocks.clone();
        let mut scratch = loader.scratches.pop().unwrap_or_else(Scratch::new);
        loader.sections.start_meshing(at);
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

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_mesh::StreamSpan;

    fn loader() -> Loader {
        Loader::new(&Budget {
            quads: 1 << 12,
            models: 1 << 12,
            faces: 1 << 16,
            groups: 1 << 12,
            sections: 1 << 8,
            tint_size: [512; 2],
        })
    }

    fn adopt(loader: &mut Loader, store: &mut ColumnStore, cave: &mut CaveCull) {
        let mut catalog = BlockCatalog::new();
        let mut tints = ColumnTints::new([512; 2]);
        store.drain_changes(&mut loader.changes);
        loader.adopt(store, blocks::corpus(), &mut catalog, &mut tints, cave);
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
            faces: vec![[0; 2]; 4],
            complex: Vec::new(),
            groups: vec![Group {
                quad_base: 0,
                quad_count: quads,
                section: slot,
                face: 0,
                quad_prefix: 0,
            }],
            spans,
            connectivity: mcrs_minecraft_mesh::OPEN,
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
    fn a_stream_needing_the_whole_arena_still_draws_every_section() {
        let mut loader = Loader::new(&Budget {
            quads: 1 << 12,
            models: 1 << 12,
            faces: 1 << 16,
            groups: 1 << 12,
            sections: 1 << 12,
            tint_size: [512; 2],
        });
        let mut cave = CaveCull::new(1 << 12);

        // Past a thousand records the ask is a class the arena only has one of, so the
        // block the stream leaves has to be the room the next one comes out of.
        let placed = 3000u32;
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

    /// A mesh whose records land in the three greedy streams, so many at a time.
    fn greedy_groups(section: [i32; 3], slot: u32, counts: [u32; 3]) -> SectionMesh {
        let mut spans = [StreamSpan::default(); STREAMS];
        for (index, count) in counts.iter().enumerate() {
            spans[index * 2] = StreamSpan {
                group_count: *count,
                quad_count: *count,
            };
        }
        let total = counts.iter().sum::<u32>();
        SectionMesh {
            section,
            simple: vec![[0; QUAD_WORDS]; total as usize],
            faces: vec![[0; 2]; 4],
            complex: Vec::new(),
            groups: (0..total)
                .map(|quad| Group {
                    quad_base: quad,
                    quad_count: 1,
                    section: slot,
                    face: 0,
                    quad_prefix: 0,
                })
                .collect(),
            spans,
            connectivity: mcrs_minecraft_mesh::OPEN,
        }
    }

    #[test]
    fn streams_growing_together_reach_every_record_the_arena_has_room_for() {
        let mut loader = Loader::new(&Budget {
            quads: 1 << 12,
            models: 1 << 12,
            faces: 1 << 14,
            groups: 1 << 12,
            sections: 1 << 10,
            tint_size: [512; 2],
        });
        let mut cave = CaveCull::new(1 << 10);

        // Streams growing at their own rates leave the arena holding its room in pieces: at
        // 342 sections these three want 256, 1024 and 2048 records, which is 3584 of the 4096
        // the arena has and more than any one piece of what is left holds.
        let counts = [1u32, 3, 2];
        let placed = 342u32;
        for index in 0..placed {
            loader
                .place(
                    greedy_groups([index as i32, 0, 0], index, counts),
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
            [
                draws[0].group_count,
                draws[2].group_count,
                draws[4].group_count
            ],
            counts.map(|count| count * placed),
            "every record placed is one the draw reaches"
        );
    }

    #[test]
    fn a_stream_the_arena_cannot_hold_stops_asking_instead_of_sending_itself_again() {
        let mut loader = Loader::new(&Budget {
            quads: 1 << 13,
            models: 1 << 12,
            faces: 1 << 16,
            groups: 1 << 12,
            sections: 1 << 13,
            tint_size: [512; 2],
        });
        let mut cave = CaveCull::new(1 << 13);

        let placed = 5000u32;
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

        let flushed = loader
            .flush()
            .expect("a flush hands over the whole draw list");
        assert_eq!(
            flushed.draws.expect("the whole draw list")[0].group_count,
            1 << 12,
            "a stream past what the arena can ever give it draws what its block holds"
        );
        assert!(
            flushed.groups.is_empty(),
            "and stops packing and sending itself again for the room that is not coming"
        );
    }

    #[test]
    fn a_column_is_meshed_only_once_every_column_it_borders_has_arrived() {
        use crate::columns::{Column, Extent};

        let loader = loader();
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
        let mut sections = Sections::default();
        let camera = [0, 0, 0];
        for at in [[40, 0, 0], [3, 0, 0], [12, 0, 0], [-7, 0, 0]] {
            sections.enqueue(at, camera);
        }
        sections.dequeue([3, 0, 0]);
        assert_eq!(sections.pop_nearest(camera), Some([-7, 0, 0]));
        assert_eq!(
            sections.pop_nearest([36, 0, 0]),
            Some([40, 0, 0]),
            "the camera moved past the step, so the order follows it"
        );
        assert_eq!(sections.pop_nearest([36, 0, 0]), Some([12, 0, 0]));
        assert_eq!(sections.pop_nearest([36, 0, 0]), None);
        assert_eq!(sections.queued(), 0);
    }

    #[test]
    fn a_section_waiting_for_room_is_queued_again_but_one_being_meshed_is_not() {
        let mut sections = Sections::default();
        let (waiting, meshing) = ([1, 0, 0], [2, 0, 0]);
        sections.defer(waiting);
        sections.start_meshing(meshing);

        sections.enqueue(meshing, [0; 3]);
        sections.requeue_deferred([0; 3]);
        assert!(sections.is_queued(waiting));
        assert!(sections.is_meshing(meshing));
        assert_eq!(sections.queued(), 1);
        assert_eq!(sections.pop_nearest([0; 3]), Some(waiting));
        assert_eq!(sections.pop_nearest([0; 3]), None);
    }

    #[test]
    fn a_section_is_queued_when_the_last_of_its_neighbours_arrives_not_when_it_does() {
        use crate::columns::{Column, Extent, Section};

        let extent = Extent {
            min_section_y: 0,
            sections: 1,
        };
        let stone = || {
            Column::unlit(
                0,
                vec![Some(Section {
                    blocks: Box::new([1; crate::columns::SECTION_VOLUME]),
                    biomes: Box::new([0; crate::columns::BIOME_CELLS]),
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
        adopt(&mut loader, &mut store, &mut cave);
        assert_eq!(
            loader.sections.queued(),
            0,
            "no column is surrounded yet, so nothing is meshable"
        );

        store.insert(last, stone());
        adopt(&mut loader, &mut store, &mut cave);
        assert!(
            loader.sections.is_queued([0, 0, 0]),
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
    fn a_mesh_read_out_of_a_column_the_server_took_back_is_thrown_away() {
        use crate::columns::{Column, Extent, Section};

        let stone = || {
            Column::unlit(
                0,
                vec![Some(Section {
                    blocks: Box::new([1; crate::columns::SECTION_VOLUME]),
                    biomes: Box::new([0; crate::columns::BIOME_CELLS]),
                    states: vec![1],
                })],
            )
        };

        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8);
        let mut store = ColumnStore::default();
        store.enter(Extent {
            min_section_y: 0,
            sections: 1,
        });
        for x in -1..=1 {
            for z in -1..=1 {
                store.insert(ColumnPos::new(x, z), stone());
            }
        }
        adopt(&mut loader, &mut store, &mut cave);

        let centre = [0, 0, 0];
        let slot = loader.take_slot().expect("a free slot");
        loader.sections.dequeue(centre);
        loader.sections.start_meshing(centre);

        store.remove(ColumnPos::new(0, 0));
        adopt(&mut loader, &mut store, &mut cave);
        assert!(
            loader.discard_orphan(centre, slot, &store),
            "the mesh was read out of a column nobody holds any more"
        );
        assert!(!loader.sections.is_resident(centre), "so it is not drawn");
        assert_eq!(loader.free_slots, vec![slot], "and its slot goes back");
        assert!(
            !loader.sections.is_queued(centre),
            "with the column gone there is nothing to mesh again"
        );

        // The same again, only the server sends the column back before the mesh lands.
        loader.sections.start_meshing(centre);
        store.insert(ColumnPos::new(0, 0), stone());
        store.remove(ColumnPos::new(0, 0));
        store.insert(ColumnPos::new(0, 0), stone());
        adopt(&mut loader, &mut store, &mut cave);
        assert!(
            !loader.sections.is_queued(centre),
            "the arrival cannot queue a section a task still holds"
        );
        assert!(loader.discard_orphan(centre, slot, &store));
        assert!(
            loader.sections.is_queued(centre),
            "the column is back, so the section is meshed again out of what it holds now"
        );
    }

    #[test]
    fn a_column_taken_back_before_it_is_adopted_leaves_the_section_count_where_it_was() {
        use crate::columns::{Column, Extent, Section};

        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8);
        let mut store = ColumnStore::default();
        store.enter(Extent {
            min_section_y: 0,
            sections: 1,
        });

        let pos = ColumnPos::new(0, 0);
        store.insert(
            pos,
            Column::unlit(
                0,
                vec![Some(Section {
                    blocks: Box::new([1; crate::columns::SECTION_VOLUME]),
                    biomes: Box::new([0; crate::columns::BIOME_CELLS]),
                    states: vec![1],
                })],
            ),
        );
        store.remove(pos);
        adopt(&mut loader, &mut store, &mut cave);

        assert_eq!(
            loader.sections_total, 0,
            "a column the server took back before the frame that adopts it counts for as much \
             on the way out as it did on the way in"
        );
    }

    #[test]
    fn no_section_is_meshed_before_the_catalog_reaches_the_states_it_holds() {
        let mut catalog = BlockCatalog::new();
        assert!(!catalog.caught_up(), "nothing has been baked yet");

        catalog.blocks = Arc::new(vec![BlockInfo::default()]);
        assert!(catalog.caught_up());

        catalog.to_bake.push(7);
        assert!(
            !catalog.caught_up(),
            "a state the catalog still owes holds the mesher back"
        );
    }

    #[test]
    fn a_column_wraps_into_the_tint_window_wherever_it_is() {
        let tints = ColumnTints::new([512; 2]);
        assert_eq!(tints.corner(ColumnPos::new(0, 0)), [0, 0]);
        assert_eq!(tints.corner(ColumnPos::new(31, 1)), [496, 16]);
        assert_eq!(tints.corner(ColumnPos::new(32, 0)), [0, 0]);
        assert_eq!(tints.corner(ColumnPos::new(-1, -33)), [496, 496]);
        assert_eq!(tints.corner(ColumnPos::new(1_000_000, 0)), [0, 0]);
    }
}

#[cfg(test)]
mod mesh_queue_tests {
    use super::*;

    /// A section requeued after its first entry was left behind sits in the
    /// buckets twice, so the rebucket has to count what it actually holds.
    #[test]
    fn a_requeued_section_does_not_underflow_the_entry_count() {
        let mut sections = Sections::default();
        let requeued = [4, 0, 0];
        let other = [5, 0, 0];
        sections.enqueue(requeued, [0; 3]);
        sections.dequeue(requeued);
        sections.enqueue(requeued, [0; 3]);
        sections.enqueue(other, [0; 3]);
        sections.rebucket([0; 3]);
        assert_eq!(sections.pop_nearest([0; 3]), Some(requeued));
        assert_eq!(sections.pop_nearest([0; 3]), Some(other));
        assert_eq!(sections.pop_nearest([0; 3]), None);
    }
}
