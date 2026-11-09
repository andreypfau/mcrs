use std::collections::BinaryHeap;
use std::sync::Arc;

use bevy::ecs::system::SystemParam;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures::check_ready};
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_client_world::columns::{
    ClientTerrainSet, ColumnChange, ColumnStore, Columns, Extent,
};
#[cfg(feature = "dev")]
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, TraceEvent};
use mcrs_minecraft_mesh::SECTION_SIZE;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::{BlockPos, ColumnPos, LocalPos, SectionPos};

use crate::cave::{CaveCull, NO_SLOT};
use crate::tint::{self, BiomeTints};
use crate::vanilla::VanillaAssets;
use mcrs_minecraft_client_world::joined_game::CurrentDimension;
use mcrs_minecraft_client_world::registries::{SessionSet, entered_configuration};
use mcrs_minecraft_dimension::CardinalLight;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_mesh::arena::{Arena, Block};
use mcrs_minecraft_mesh::block::BlockInfo;
use mcrs_minecraft_mesh::pack::QUAD_WORDS;
use mcrs_minecraft_mesh::smooth_light::SmoothLight;
use mcrs_minecraft_mesh::{self as mesh, Connectivity, Draw, Group, STREAMS, Scratch, SectionMesh};
use mcrs_minecraft_model::atlas::SpriteArray;
use mcrs_minecraft_model::block_catalog::{self, Catalog};
use mcrs_minecraft_model::item_model_bake::{ItemModels, bake_all as bake_items};
use mcrs_minecraft_model::resource_set::{PackLoadError, ResourceSet};
use mcrs_minecraft_render::{
    Animation, AtlasUpdate, Budget, FACE_BYTES, Placement, STILL, SectionDesc, SpriteEntry,
    SpriteUpload, TerrainShade, Upload, Uploads,
};
use mcrs_minecraft_resource::Id;

const HYSTERESIS: f32 = (16 * SECTION_SIZE) as f32;

/// The options of the client that change how the terrain mesh is lit.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightOptions {
    pub smooth_light: SmoothLight,
}

#[derive(Resource, Clone, Copy)]
pub struct MeshPacing {
    pub per_frame: usize,
    pub in_flight: usize,
}

pub struct StreamPlugin {
    budget: Arc<Budget>,
    uploads: Uploads,
    pacing: MeshPacing,
}

impl StreamPlugin {
    pub fn new(budget: Arc<Budget>, uploads: Uploads, pacing: MeshPacing) -> Self {
        Self {
            budget,
            uploads,
            pacing,
        }
    }
}

impl Plugin for StreamPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Loader::new(&self.budget))
            .insert_resource(BlockCatalog::new())
            .insert_resource(ColumnTints::new(self.budget.tint_size))
            .insert_resource(self.uploads.clone())
            .insert_resource(self.pacing)
            .init_resource::<LightOptions>()
            .init_resource::<TerrainShade>()
            .add_systems(
                Update,
                apply_dimension_shade
                    .before(ClientTerrainSet::Build)
                    .run_if(resource_exists::<CurrentRegistries>),
            )
            .add_systems(
                Update,
                (
                    follow_camera,
                    remesh_on_light_options,
                    adopt_columns,
                    bake_catalog.run_if(in_state(VanillaAssets::Ready)),
                    tint_columns,
                    place_meshes,
                    flush_streams,
                    admit_meshing,
                )
                    .chain()
                    .in_set(ClientTerrainSet::Build)
                    .run_if(can_stream),
            )
            .add_systems(
                Update,
                clear_biome_tints
                    .in_set(SessionSet::Clear)
                    .run_if(entered_configuration),
            )
            .add_systems(
                Update,
                derive_biome_tints
                    .in_set(SessionSet::Derive)
                    .after(bake_catalog)
                    .before(tint_columns)
                    .run_if(resource_exists::<CurrentRegistries>),
            )
            .add_systems(
                Update,
                collect_column_changes
                    .in_set(ClientTerrainSet::Build)
                    .before(adopt_columns)
                    .run_if(resource_exists::<ColumnStore>),
            );
    }
}

type MeshingSection = ([i32; 3], u32, Task<(SectionMesh, Scratch)>);
type FarthestResidents = ([i32; 3], BinaryHeap<(u32, [i32; 3])>);

/// The sections the server's columns hold, from queued to drawn, and the arena they are
/// drawn out of.
#[derive(Resource)]
pub struct Loader {
    changes: Vec<ColumnChange>,
    columns: usize,
    sections_total: usize,
    sections: Sections,
    /// Sections whose light or blocks a delta rewrote, present or not.
    stale: HashSet<[i32; 3]>,
    /// Geometry of the meshes placed since the last flush. The flush queues it behind the kills
    /// of the meshes whose room it reuses.
    geometry: Vec<Placement>,
    /// Meshes replaced since the last flush. The GPU draws them until the records of their
    /// replacements land, so their records die, and their room and slot come back, only behind
    /// those records.
    replaced: Vec<Resident>,
    /// Group blocks of rebuilt streams, which the GPU draws until the new draw list lands.
    retired_blocks: Vec<Block>,
    meshing: Vec<MeshingSection>,
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
    /// Residents farthest first, as of the camera section it was built in. Entries go stale as
    /// sections leave and are skipped when they surface.
    farthest: Option<FarthestResidents>,
    quads: Arena,
    models: Arena,
    faces: Arena,
    groups: Arena,
    #[cfg(feature = "dev")]
    pub(crate) trace: Vec<TraceEvent>,
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
    Loading(Task<Result<ResourceSet, PackLoadError>>),
    Ready(Arc<ResourceSet>),
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

/// A section's drawn mesh and what the pipeline does for it, apart: a relit section stays
/// drawn while it is queued and meshed again.
#[derive(Default)]
struct Section {
    resident: Option<Resident>,
    work: Work,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Work {
    #[default]
    Idle,
    Queued,
    /// On the compute pool. An orphaned mesh was read out of a column that left or was sent
    /// again meanwhile, so the world it holds is one nobody holds any more.
    Meshing {
        orphaned: bool,
    },
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
    /// The arena had no room for a new block beside the one the GPU draws, so the stream
    /// draws what its block holds; set to warn once.
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
            stale: HashSet::new(),
            geometry: Vec::new(),
            replaced: Vec::new(),
            retired_blocks: Vec::new(),
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
            farthest: None,
            quads: Arena::new(budget.quads),
            models: Arena::new(budget.models),
            faces: Arena::new(budget.faces),
            groups: Arena::new(budget.groups),
            #[cfg(feature = "dev")]
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

    /// Holds blocks, is not on the pool, and borders only columns that have arrived.
    fn meshable(&self, at: [i32; 3], store: &ColumnStore) -> bool {
        !self.sections.is_meshing(at)
            && store
                .stored_section(SectionPos::new(at[0], at[1], at[2]))
                .is_some()
            && self.surrounded(ColumnPos::new(at[0], at[2]), store)
    }

    fn camera_section(&self) -> [i32; 3] {
        (self.camera / SECTION_SIZE as f32)
            .floor()
            .as_ivec3()
            .to_array()
    }

    fn look_from(&mut self, eye: Vec3) {
        let was = self.camera_section();
        self.camera = eye;
        if self.camera_section() == was {
            return;
        }
        for (index, stream) in self.streams.iter_mut().enumerate() {
            if blended(index) && stream.live() != 0 {
                stream.rebuild = true;
                self.dirty = true;
            }
        }
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
                    if self.sections.resident(at).is_none() && self.meshable(at, store) {
                        self.enqueue(at);
                    }
                }
            }
        }
    }

    /// The faces and the shade of a block reach into the sections that touch it, across a
    /// face, an edge or a corner.
    fn mark_stale_around(&mut self, at: BlockPos) {
        let section = SectionPos::from(at);
        let local = LocalPos::from(at);
        let reach = |coordinate: u8| {
            let low = if coordinate == 0 { -1 } else { 0 };
            let high = if usize::from(coordinate) == SectionPos::MASK {
                1
            } else {
                0
            };
            low..=high
        };
        for dy in reach(local.y()) {
            for dz in reach(local.z()) {
                for dx in reach(local.x()) {
                    self.stale
                        .insert([section.x + dx, section.y + dy, section.z + dz]);
                }
            }
        }
    }

    /// A section whose light or blocks a delta rewrote has to be meshed again, and so do the
    /// six around it: a face samples the light of the cell it faces, which for a
    /// face on the section boundary lies in the neighbour. One still being meshed
    /// waits for the next frame, since the task holds the light it started with. The mesh
    /// a section has stays drawn until the new one replaces it.
    fn remesh_stale(&mut self, store: &ColumnStore, cave: &mut CaveCull) {
        for at in std::mem::take(&mut self.stale) {
            if self.sections.is_meshing(at) {
                self.stale.insert(at);
            } else if self.meshable(at, store) {
                self.enqueue(at);
            } else {
                self.evict(at, cave);
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

    fn victim(&mut self, candidate: f32) -> Option<[i32; 3]> {
        let camera = self.camera_section();
        if self
            .farthest
            .as_ref()
            .is_none_or(|(built, _)| *built != camera)
        {
            let heap = self
                .sections
                .residents()
                .map(|(at, _)| (heap_key(camera, at), at))
                .collect();
            self.farthest = Some((camera, heap));
        }
        let eye = self.camera;
        let (_, heap) = self.farthest.as_mut()?;
        while let Some(&(_, at)) = heap.peek() {
            if self.sections.resident(at).is_none() {
                heap.pop();
                continue;
            }
            return worth_evicting(distance_from(eye, at), candidate).then(|| {
                heap.pop();
                at
            });
        }
        None
    }

    fn placed(&mut self, at: [i32; 3]) {
        if let Some((camera, heap)) = &mut self.farthest {
            heap.push((heap_key(*camera, at), at));
        }
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
        if let Some(held) = self.sections.take_resident(section) {
            self.release(section, held, cave);
        }
    }

    /// Kills the records of a mesh that leaves the screen and gives its room and slot back at
    /// once: the next flush queues the kill ahead of any geometry written into the room.
    fn release(&mut self, section: [i32; 3], held: Resident, cave: &mut CaveCull) {
        if held.slot != NO_SLOT {
            self.doom(&held);
            for (stream, &(first, count)) in self.streams.iter_mut().zip(&held.groups) {
                kill(stream, first, count);
                stream.touched.push((first, first + count));
            }
            self.free_slots.push(held.slot);
        }
        self.free_room(&held);
        cave.forget(section);
        self.evicted += 1;
    }

    /// Counts a mesh's records dead before they are zeroed, so a stream they fill with dead
    /// records is rebuilt.
    fn doom(&mut self, held: &Resident) {
        for (stream, &(_, count)) in self.streams.iter_mut().zip(&held.groups) {
            stream.dead += count as usize;
            if count != 0 && stream.dead * 2 >= stream.groups.len() {
                stream.rebuild = true;
            }
        }
        self.dirty = true;
    }

    fn free_room(&mut self, held: &Resident) {
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.faces.free(held.faces);
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
    #[expect(
        clippy::excessive_nesting,
        reason = "marks every section around an arrived column for relighting across the three axes, so the diagonal neighbours stay beside the arrival that makes them stale"
    )]
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
                    #[cfg(feature = "dev")]
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
                    #[cfg(feature = "dev")]
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
                            catalog.owe(state.number());
                        }
                    }
                    if let Some(extent) = extent {
                        self.enqueue_around(pos, extent, store);
                    }
                }
                ColumnChange::Filled(_) => self.sections_total += 1,
                ColumnChange::Reblocked(at, state) => {
                    catalog.owe(state.number());
                    self.mark_stale_around(at);
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
                                    self.stale.insert([pos.x + dx, sy + dy, pos.z + dz]);
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

    /// Places a mesh in the arena, in place of the one its section held, or hands it back
    /// when there is no room.
    fn place(
        &mut self,
        mesh: SectionMesh,
        slot: u32,
        cave: &mut CaveCull,
    ) -> Result<(), Box<SectionMesh>> {
        let Some([quads, models, faces]) = self.reserve(&mesh) else {
            return Err(Box::new(mesh));
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
                stream.quads_end += group.quad_count;
            }
            if run != 0 {
                let at = stream.groups.len() as u32;
                stream.groups.extend_from_slice(&placed[first..first + run]);
                ranges[index] = (at, run as u32);
                stream.touched.push((at, at + run as u32));
                if blended(index) || stream.groups.len() > stream.block.capacity() {
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
            self.geometry.push(Placement {
                quads: ((quads.offset * QUAD_WORDS * 4) as u64, mesh.simple),
                vertices: ((models.offset * 4 * 3 * 4) as u64, mesh.complex),
                faces: ((faces.offset * FACE_BYTES) as u64, mesh.faces),
                sections: (
                    (slot as usize * size_of::<SectionDesc>()) as u64,
                    vec![SectionDesc {
                        section,
                        scale: 1,
                        face_base: faces.offset as u32,
                    }],
                ),
                ..Placement::default()
            });
            slot
        };
        cave.set_section(section, slot, mesh.connectivity);
        #[cfg(feature = "dev")]
        self.trace.push(TraceEvent::mark(
            ColumnPos::new(section[0], section[2]),
            ColumnStage::Meshed,
        ));
        let replaced = self.sections.settle(
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
        if let Some(held) = replaced {
            if held.slot == NO_SLOT {
                self.free_room(&held);
            } else {
                self.doom(&held);
                self.replaced.push(held);
            }
        }
        Ok(())
    }

    /// The touched records of every stream as runs to write into its block. What lies past a
    /// stream's block waits for the rebuild that reaches it.
    fn touched_runs(&mut self) -> Vec<(u64, Vec<Group>)> {
        let mut runs = Vec::new();
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
            for (first, end) in merged {
                stream.write(first, end, &mut runs);
            }
            stream.touched = ranges;
        }
        runs
    }

    fn draws(&self) -> Vec<Draw> {
        self.streams
            .iter()
            .enumerate()
            .map(|(index, stream)| Draw {
                stream: index as u32,
                first_group: stream.block.offset as u32,
                group_count: stream.groups.len().min(stream.block.capacity()) as u32,
                quad_count: stream.quads_end,
            })
            .collect()
    }

    /// Queues what changed since the last flush so that the GPU never draws a hole where a
    /// mesh was replaced, nor a record over quads another mesh wrote. The uploads land one
    /// after another, each cut anywhere by the per-frame budget:
    ///
    /// 1. The touched records, killing released meshes before their room is written again.
    ///    New records lie past the draw list the GPU holds, so they draw nothing yet.
    /// 2. The geometry of the meshes placed.
    /// 3. Rebuilt streams in fresh blocks, and the draw list, which the GPU swaps in only once
    ///    the whole upload has landed.
    /// 4. The kills of replaced meshes, which the GPU draws until the swap.
    ///
    /// Replaced room and slots and the blocks rebuilt streams left come back once all of it
    /// is queued.
    fn flush_to(&mut self, uploads: &Uploads) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let touched = self.touched_runs();
        if !touched.is_empty() {
            uploads.push(Upload::Geometry(Placement {
                groups: touched,
                ..Placement::default()
            }));
        }
        for placement in self.geometry.drain(..) {
            uploads.push(Upload::Geometry(placement));
        }
        self.rebuild_blocks();
        uploads.push(Upload::Geometry(Placement {
            groups: self.touched_runs(),
            draws: Some(self.draws()),
            ..Placement::default()
        }));
        let mut replaced = std::mem::take(&mut self.replaced);
        for held in replaced.drain(..) {
            for (stream, &(first, count)) in self.streams.iter_mut().zip(&held.groups) {
                kill(stream, first, count);
                stream.touched.push((first, first + count));
            }
            self.free_room(&held);
            self.free_slots.push(held.slot);
        }
        self.replaced = replaced;
        let kills = self.touched_runs();
        if !kills.is_empty() {
            uploads.push(Upload::Geometry(Placement {
                groups: kills,
                ..Placement::default()
            }));
        }
        for block in self.retired_blocks.drain(..) {
            self.groups.free(block);
        }
    }

    /// Places a landed mesh, making room by evicting the farthest resident, or for a relit
    /// section in a full arena by giving up the mesh it replaces, or leaves the section waiting
    /// for room.
    fn land(&mut self, mesh: SectionMesh, slot: u32, cave: &mut CaveCull) {
        let at = mesh.section;
        let here = self.distance(at);
        let mut pending = mesh;
        loop {
            let back = match self.place(pending, slot, cave) {
                Ok(()) => {
                    self.placed(at);
                    return;
                }
                Err(back) => back,
            };
            pending = *back;
            if let Some(victim) = self.victim(here) {
                // The victim is the farthest resident, so meshing it again could only end
                // deferred; it waits for the camera to move instead.
                self.evict(victim, cave);
                self.sections.defer(victim);
            } else if let Some(own) = self.sections.take_resident(at) {
                self.release(at, own, cave);
            } else {
                self.sections.defer(at);
                self.free_slots.push(slot);
                return;
            }
        }
    }

    /// Gives a fresh block to every stream that outgrew the one it has or filled it with dead
    /// records. The GPU draws the old block until the new draw list lands, so the new block is
    /// taken beside it, and a stream the arena has no such room for draws what its block holds.
    fn rebuild_blocks(&mut self) {
        for index in 0..STREAMS {
            let stream = &mut self.streams[index];
            if !stream.rebuild {
                continue;
            }
            let Some(block) = self.groups.alloc(stream.live().max(MIN_BLOCK)) else {
                if !stream.starved {
                    stream.starved = true;
                    warn!(
                        stream = index,
                        live = stream.live(),
                        held = self.groups.held(),
                        capacity = self.groups.capacity(),
                        "the group arena has no room to rebuild this stream; it draws only what \
                         its block holds and the rest of its sections stay off the screen"
                    );
                }
                continue;
            };
            stream.starved = false;
            self.retired_blocks
                .push(std::mem::replace(&mut stream.block, block));
            // A replaced mesh leaves a rebuilt stream with the old block rather than by a kill.
            for held in &mut self.replaced {
                let (first, count) = std::mem::take(&mut held.groups[index]);
                kill(stream, first, count);
            }
            self.pack_stream(index);
        }
    }

    /// Packs a stream's live records into the block it holds, keeping their order, or for a
    /// blended stream putting the farthest section first, and telling each resident section
    /// where its records went.
    fn pack_stream(&mut self, index: usize) {
        let camera = IVec3::from_array(self.camera_section());
        let Self {
            streams,
            owners,
            sections,
            ..
        } = self;
        let stream = &mut streams[index];
        let live = stream.live();
        let mut packed: Vec<Group> = std::mem::take(&mut stream.groups)
            .into_iter()
            .filter(|group| group.quad_count != 0)
            .collect();
        if blended(index) {
            // Ties go by position so the order never depends on which section landed first.
            packed.sort_by_key(|group| {
                let at = IVec3::from_array(owners[group.section as usize]);
                let distance = (at - camera).as_i64vec3().length_squared();
                (std::cmp::Reverse(distance), at.to_array())
            });
        }
        let mut quads = 0u32;
        let mut last_slot = NO_SLOT;
        for (at, group) in packed.iter_mut().enumerate() {
            if let Some(resident) = sections.resident_mut(owners[group.section as usize]) {
                if group.section != last_slot {
                    resident.groups[index] = (at as u32, 0);
                }
                resident.groups[index].1 += 1;
            }
            last_slot = group.section;
            quads += group.quad_count;
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

    /// Adds the records from `first` to `end` as a run at their place in the block.
    fn write(&self, first: u32, end: u32, runs: &mut Vec<(u64, Vec<Group>)>) {
        let Some(records) = self.groups.get(first as usize..end as usize) else {
            return;
        };
        let offset = (self.block.offset + first as usize) * size_of::<Group>();
        runs.push((offset as u64, records.to_vec()));
    }
}

/// Stops drawing the records from `first` on. Only the count is zeroed, so a record the
/// budget cuts in two reads either its own quads or none.
fn kill(stream: &mut Stream, first: u32, count: u32) {
    if let Some(groups) = stream
        .groups
        .get_mut(first as usize..first as usize + count as usize)
    {
        for group in groups {
            group.quad_count = 0;
        }
    }
}

/// The GUI textures and translations come from the pack the catalog bakes from, once it has
/// loaded.
pub fn load_gui_atlas(catalog: Res<BlockCatalog>, mut commands: Commands) {
    if let Some(pack) = catalog.pack()
        && let Err(reason) = mcrs_minecraft_client_gui::scene::load_gui_atlas(pack, &mut commands)
    {
        panic!("cannot load the GUI: {reason}");
    }
}

impl BlockCatalog {
    pub fn pack(&self) -> Option<&Arc<ResourceSet>> {
        match &self.pack {
            PackLoad::Ready(pack) => Some(pack),
            _ => None,
        }
    }

    fn new() -> Self {
        Self {
            pack: PackLoad::Pending,
            catalog: Some(block_catalog::empty()),
            blocks: Arc::new(Vec::new()),
            baked: Vec::new(),
            to_bake: Vec::new(),
            baking: None,
            foreign_states: false,
            failures: 0,
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
    fn poll_pack(&mut self, assets: &AssetServer) -> Option<Arc<ResourceSet>> {
        match &mut self.pack {
            PackLoad::Pending => {
                let assets = assets.clone();
                self.pack = PackLoad::Loading(
                    IoTaskPool::get().spawn(async move { ResourceSet::load(&assets).await }),
                );
                None
            }
            PackLoad::Loading(task) => {
                let pack = check_ready(task)?
                    .unwrap_or_else(|reason| panic!("cannot read the resource pack: {reason}"));
                info!(files = pack.len(), "read the resource pack");
                for error in pack.block_entity_models().errors() {
                    warn!("block entity content error: {error}");
                }
                let pack = Arc::new(pack);
                self.pack = PackLoad::Ready(pack.clone());
                Some(pack)
            }
            PackLoad::Ready(pack) => Some(pack.clone()),
        }
    }

    fn start_baking(
        &mut self,
        pack: &Arc<ResourceSet>,
        definitions: &Blocks,
        pool: &'static AsyncComputeTaskPool,
    ) {
        let Some(mut catalog) = self.catalog.take() else {
            return;
        };
        let states = std::mem::take(&mut self.to_bake);
        let known = self.sprites;
        let sent = self.sent.clone();
        let definitions = definitions.clone();
        let pack = pack.clone();
        let bake_items_too = !self.items_baked;
        self.items_baked = true;
        self.baking = Some(pool.spawn(async move {
            block_catalog::extend(&pack, &mut catalog, &definitions, &states);
            let items = bake_items_too.then(|| {
                let started = bevy::platform::time::Instant::now();
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
    states: HashMap<[i32; 3], Section>,
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

impl Sections {
    fn work(&self, at: [i32; 3]) -> Work {
        self.states
            .get(&at)
            .map_or(Work::Idle, |section| section.work)
    }

    fn set_work(&mut self, at: [i32; 3], work: Work) {
        let entry = self.states.entry(at).or_default();
        match entry.work {
            Work::Queued => self.tally.queued -= 1,
            Work::Deferred => {
                self.tally.deferred.remove(&at);
            }
            Work::Idle | Work::Meshing { .. } => {}
        }
        match work {
            Work::Queued => self.tally.queued += 1,
            Work::Deferred => {
                self.tally.deferred.insert(at);
            }
            Work::Idle | Work::Meshing { .. } => {}
        }
        entry.work = work;
        self.prune(at);
    }

    fn prune(&mut self, at: [i32; 3]) {
        if self
            .states
            .get(&at)
            .is_some_and(|section| section.work == Work::Idle && section.resident.is_none())
        {
            self.states.remove(&at);
        }
    }

    fn queued(&self) -> usize {
        self.tally.queued
    }

    fn is_meshing(&self, at: [i32; 3]) -> bool {
        matches!(self.work(at), Work::Meshing { .. })
    }

    fn resident(&self, at: [i32; 3]) -> Option<&Resident> {
        self.states.get(&at)?.resident.as_ref()
    }

    fn resident_mut(&mut self, at: [i32; 3]) -> Option<&mut Resident> {
        self.states.get_mut(&at)?.resident.as_mut()
    }

    fn residents(&self) -> impl Iterator<Item = ([i32; 3], &Resident)> {
        self.states
            .iter()
            .filter_map(|(at, section)| Some((*at, section.resident.as_ref()?)))
    }

    /// Only a section that is not queued or on the pool can be queued.
    fn enqueue(&mut self, at: [i32; 3], camera: [i32; 3]) {
        if !matches!(self.work(at), Work::Idle | Work::Deferred) {
            return;
        }
        self.set_work(at, Work::Queued);
        self.queue.push(at, camera);
    }

    #[cfg(test)]
    fn dequeue(&mut self, at: [i32; 3]) {
        if self.work(at) == Work::Queued {
            self.set_work(at, Work::Idle);
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
            states
                .get(&at)
                .is_some_and(|section| section.work == Work::Queued)
        })?;
        self.set_work(at, Work::Idle);
        Some(at)
    }

    fn start_meshing(&mut self, at: [i32; 3]) {
        self.set_work(at, Work::Meshing { orphaned: false });
    }

    /// Ends a section's meshing, telling whether the mesh was orphaned on the way.
    fn finish_meshing(&mut self, at: [i32; 3]) -> Option<bool> {
        let Work::Meshing { orphaned } = self.work(at) else {
            return None;
        };
        self.set_work(at, Work::Idle);
        Some(orphaned)
    }

    /// Makes `held` the section's drawn mesh, handing back the one it replaces.
    fn settle(&mut self, at: [i32; 3], held: Resident) -> Option<Resident> {
        let replaced = self.states.entry(at).or_default().resident.replace(held);
        if replaced.is_none() {
            self.tally.residents += 1;
        }
        replaced
    }

    /// A section that is queued or on the pool goes on with that instead.
    fn defer(&mut self, at: [i32; 3]) {
        if self.work(at) == Work::Idle {
            self.set_work(at, Work::Deferred);
        }
    }

    fn take_resident(&mut self, at: [i32; 3]) -> Option<Resident> {
        let held = self.states.get_mut(&at)?.resident.take()?;
        self.tally.residents -= 1;
        self.prune(at);
        Some(held)
    }

    /// The section's column left: it stops waiting, and a mesh still being read out of it is
    /// orphaned. A resident mesh is the caller's to evict.
    fn depart(&mut self, at: [i32; 3]) {
        match self.work(at) {
            Work::Meshing { .. } => self.set_work(at, Work::Meshing { orphaned: true }),
            Work::Queued | Work::Deferred => self.set_work(at, Work::Idle),
            Work::Idle => {}
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

/// Measured from the middle of the camera's section rather than the camera itself, so the order
/// does not depend on where inside that section the camera was when a key was taken. Distances
/// are never negative, so their bit patterns order the same way they do.
fn heap_key(camera_section: [i32; 3], section: [i32; 3]) -> u32 {
    let middle = (Vec3::from_array(camera_section.map(|n| n as f32)) + 0.5) * SECTION_SIZE as f32;
    distance_from(middle, section).to_bits()
}

fn blended(stream: usize) -> bool {
    mesh::stream_pass(stream as u32).translucent()
}

fn distance_from(camera: Vec3, section: [i32; 3]) -> f32 {
    let min = Vec3::from_array(section.map(|n| (n * SECTION_SIZE as i32) as f32));
    let max = min + Vec3::splat(SECTION_SIZE as f32);
    (camera.clamp(min, max) - camera).length()
}

pub(crate) fn can_stream(
    store: Option<Res<ColumnStore>>,
    cameras: Query<(), With<Camera3d>>,
) -> bool {
    store.is_some() && cameras.single().is_ok()
}

fn follow_camera(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let loader = &mut *loader;
    loader.look_from(camera.translation());
    loader.follow(&mut cave);
    if loader.camera.distance(loader.anchor) > HYSTERESIS {
        loader.anchor = loader.camera;
        loader.requeue_deferred();
    }
}

fn collect_column_changes(mut changes: MessageReader<ColumnChange>, mut loader: ResMut<Loader>) {
    loader.changes.extend(changes.read().cloned());
}

fn adopt_columns(
    mut loader: ResMut<Loader>,
    mut catalog: ResMut<BlockCatalog>,
    mut tints: ResMut<ColumnTints>,
    mut cave: ResMut<CaveCull>,
    store: Res<ColumnStore>,
    definitions: Res<Blocks>,
) {
    if loader.changes.is_empty() {
        return;
    }
    let _adopting = info_span!("stream adopt").entered();
    loader.adopt(&store, &definitions, &mut catalog, &mut tints, &mut cave);
}

fn bake_catalog(
    mut catalog: ResMut<BlockCatalog>,
    uploads: Res<Uploads>,
    assets: Res<AssetServer>,
    definitions: Res<Blocks>,
    mut commands: Commands,
) {
    let catalog = &mut *catalog;
    let pack = catalog.poll_pack(&assets);
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
    {
        catalog.start_baking(&pack, &definitions, AsyncComputeTaskPool::get());
    }
}

/// The tint table follows the session's biomes: it is rebuilt whole when the set is replaced,
/// and built from the first set once the pack is in, since the colormaps are read from it.
fn derive_biome_tints(
    registries: Res<CurrentRegistries>,
    catalog: Res<BlockCatalog>,
    mut due: Local<bool>,
    mut commands: Commands,
) {
    *due |= registries.is_changed();
    if !*due {
        return;
    }
    let Some(pack) = catalog.pack() else {
        return;
    };
    *due = false;
    match tint::derive_tints(pack, &registries) {
        Some((tints, failures)) => {
            for failure in &failures {
                warn!("no tint for {failure}");
            }
            commands.insert_resource(tints);
        }
        None => {
            error!("the session registries hold no biomes to tint columns by");
            commands.remove_resource::<BiomeTints>();
        }
    }
}

/// The tint table holds the numbers of the set it was derived from, so a new set drops it.
fn clear_biome_tints(mut commands: Commands) {
    commands.remove_resource::<BiomeTints>();
}

fn tint_columns(
    mut tints: ResMut<ColumnTints>,
    palette: Option<Res<BiomeTints>>,
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
    let Some(palette) = palette.filter(|palette| !palette.is_empty()) else {
        return;
    };
    let pool = AsyncComputeTaskPool::get();
    let palette = BiomeTints::clone(&palette);
    for pos in std::mem::take(&mut tints.to_tint) {
        let corner = tints.corner(pos);
        let world = store.around(pos);
        let palette = palette.clone();
        tints.tinting.push((
            corner,
            pool.spawn(async move { tint::tint_column(&world, &palette, pos) }),
        ));
    }
}

/// A section's mesh off the pool, with the slot it was meshed for and the scratch it borrowed.
type Landed = ([i32; 3], u32, (SectionMesh, Scratch));

fn place_meshes(
    mut loader: ResMut<Loader>,
    mut cave: ResMut<CaveCull>,
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
        loader.land(mesh, slot, &mut cave);
    }

    loader.remesh_stale(&store, &mut cave);
}

pub(crate) fn flush_streams(mut loader: ResMut<Loader>, uploads: Res<Uploads>) {
    let _flushing = info_span!("stream flush").entered();
    loader.flush_to(&uploads);
}

/// The directional shade table follows the `cardinal_light` of the dimension type the player
/// is in.
fn apply_dimension_shade(
    current: Single<&CurrentDimension, Changed<CurrentDimension>>,
    registries: Res<CurrentRegistries>,
    mut shade: ResMut<TerrainShade>,
) {
    let Some(dimension_types) = registries.registry::<DimensionTypeFile>() else {
        return;
    };
    let table = Id::checked(current.dimension_type.number(), dimension_types.len())
        .and_then(|id| dimension_types.get(id))
        .map_or(CardinalLight::Default, |file| file.cardinal_light);
    shade.set_if_neq(TerrainShade(table));
}

/// A change of the light options or of the shade table meshes every drawn section again; the
/// old mesh stays drawn until the new one replaces it.
fn remesh_on_light_options(
    options: Res<LightOptions>,
    shade: Res<TerrainShade>,
    mut loader: ResMut<Loader>,
) {
    let options_changed = options.is_changed() && !options.is_added();
    let shade_changed = shade.is_changed() && !shade.is_added();
    if !options_changed && !shade_changed {
        return;
    }
    let loader = &mut *loader;
    let drawn = loader.sections.residents().map(|(at, _)| at);
    let meshing = loader.meshing.iter().map(|(at, _, _)| *at);
    let stale: Vec<[i32; 3]> = drawn.chain(meshing).collect();
    loader.stale.extend(stale);
}

fn admit_meshing(
    mut loader: ResMut<Loader>,
    catalog: Res<BlockCatalog>,
    store: Res<ColumnStore>,
    pacing: Res<MeshPacing>,
    options: Res<LightOptions>,
    shade: Res<TerrainShade>,
) {
    if !catalog.caught_up() {
        return;
    }
    let _admitting = info_span!("stream admit").entered();
    let loader = &mut *loader;
    let pool = AsyncComputeTaskPool::get();
    let room = pacing.in_flight.saturating_sub(loader.meshing.len());
    let wanted = loader.take_wanted(room.min(pacing.per_frame), &store);
    for (taken, &at) in wanted.iter().enumerate() {
        let Some(slot) = loader.take_slot() else {
            for &back in &wanted[taken..] {
                loader.enqueue(back);
            }
            break;
        };
        let world = store.around(ColumnPos::new(at[0], at[2]));
        let blocks = catalog.blocks.clone();
        let smooth_light = options.smooth_light;
        let shade = shade.0;
        let mut scratch = loader.scratches.pop().unwrap_or_default();
        loader.sections.start_meshing(at);
        loader.meshing.push((
            at,
            slot,
            pool.spawn(async move {
                let mesh = mesh::mesh_section(
                    &world,
                    &blocks,
                    at,
                    slot,
                    smooth_light,
                    shade,
                    &mut scratch,
                );
                (mesh, scratch)
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_biome_file::BiomeFile;
    use mcrs_minecraft_chunk::PalettedContainer;
    use mcrs_minecraft_client_world::registries::LastConfiguration;
    use mcrs_minecraft_game_registries::registry_load::{sent_by, test_registries};
    use mcrs_minecraft_mesh::StreamSpan;
    use mcrs_minecraft_registries::Received;

    fn session(edit: impl FnOnce(&mut Received)) -> CurrentRegistries {
        let mut configuration = LastConfiguration {
            received: sent_by(test_registries(), false),
            vanilla_known: false,
        };
        edit(&mut configuration.received);
        let (registries, _) =
            mcrs_minecraft_client_world::registries::load_registries(&configuration, None)
                .unwrap_or_else(|error| panic!("{error}"));
        CurrentRegistries::new(registries)
    }

    #[test]
    fn entering_configuration_drops_the_tint_table() {
        use mcrs_minecraft_network::ConnectionState;

        let mut app = App::new();
        app.add_systems(Update, clear_biome_tints.run_if(entered_configuration));
        app.insert_resource(BiomeTints::default());
        let connection = app.world_mut().spawn(ConnectionState::Game).id();
        app.update();
        assert!(app.world().get_resource::<BiomeTints>().is_some());

        app.world_mut()
            .entity_mut(connection)
            .insert(ConnectionState::Configuration);
        app.update();
        assert!(
            app.world().get_resource::<BiomeTints>().is_none(),
            "a new registry set renumbers the biomes the table is indexed by"
        );
    }

    mod jar {
        use super::*;
        use mcrs_minecraft_resource::ResourceLocationBuf;

        #[test]
        fn the_tint_table_follows_the_session_biomes() {
            let mut app = App::new();
            app.insert_resource(BlockCatalog::new());
            app.add_systems(
                Update,
                derive_biome_tints.run_if(resource_exists::<CurrentRegistries>),
            );
            app.update();
            assert!(app.world().get_resource::<BiomeTints>().is_none());

            let first = session(|_| {});
            app.insert_resource(first.clone());
            app.update();
            assert!(
                app.world().get_resource::<BiomeTints>().is_none(),
                "the colormaps are read from the pack, which is not in yet"
            );

            app.world_mut().resource_mut::<BlockCatalog>().pack =
                PackLoad::Ready(ResourceSet::shared_corpus());
            app.update();
            let tints = app.world().resource::<BiomeTints>().clone();
            fn biomes(set: &CurrentRegistries) -> &mcrs_minecraft_registries::Registry<BiomeFile> {
                set.registry::<BiomeFile>().unwrap()
            }
            assert_eq!(tints.len(), biomes(&first).len());

            let second = session(|received| {
                received
                    .registries
                    .iter_mut()
                    .find(|(sent, _)| sent.as_str() == "minecraft:worldgen/biome")
                    .expect("a server syncs its biomes")
                    .1
                    .reverse();
            });
            app.insert_resource(second.clone());
            app.update();
            let replaced = app.world().resource::<BiomeTints>();
            assert_eq!(replaced.len(), biomes(&second).len());

            let swamp = |set: &CurrentRegistries| {
                biomes(set)
                    .by_name(&ResourceLocationBuf::read("minecraft:swamp").unwrap())
                    .unwrap()
            };
            assert_ne!(swamp(&first), swamp(&second), "the swamp is numbered anew");
            assert_eq!(
                replaced[swamp(&second).index()],
                tints[swamp(&first).index()],
                "each biome keeps its tint at its new number"
            );
        }
    }

    fn loader() -> Loader {
        Loader::new(&Budget {
            quads: 1 << 12,
            models: 1 << 12,
            faces: 1 << 16,
            groups: 1 << 12,
            sections: 1 << 8,
            upload: 0,
            tint_size: [512; 2],
        })
    }

    fn adopt(loader: &mut Loader, store: &mut ColumnStore, cave: &mut CaveCull) {
        let mut catalog = BlockCatalog::new();
        let mut tints = ColumnTints::new([512; 2]);
        store.drain_changes(&mut loader.changes);
        loader.adopt(
            store,
            &mcrs_minecraft_engine_registries::test_corpus().0.0,
            &mut catalog,
            &mut tints,
            cave,
        );
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
            faces: vec![[0; mcrs_minecraft_mesh::pack::FACE_WORDS]; 4],
            complex: Vec::new(),
            groups: vec![Group {
                quad_base: 0,
                quad_count: quads,
                section: slot,
                face: 0,
                bounds: 0,
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
        let mut cave = CaveCull::new(1 << 8, true);
        let mut gpu = Gpu::default();

        for (slot, (at, quads)) in [([0, 0, 0], 3), ([1, 0, 0], 5)].into_iter().enumerate() {
            loader
                .place(
                    one_greedy_group(at, slot as u32, quads),
                    slot as u32,
                    &mut cave,
                )
                .unwrap_or_else(|_| panic!("the arena has room"));
        }
        gpu.flush(&mut loader, |_| {});
        assert_eq!(
            gpu.draws.len(),
            STREAMS,
            "one draw a bucket, however many sections"
        );
        assert_eq!(gpu.draws[0].quad_count, 8);
        assert_eq!(
            gpu.drawn().iter().map(|g| g.quad_count).collect::<Vec<_>>(),
            [3, 5],
            "the blend order of a bucket runs across the sections in it"
        );

        loader.evict([0, 0, 0], &mut cave);
        gpu.flush(&mut loader, |_| {});
        assert_eq!(gpu.draws[0].group_count, 1);
        assert_eq!(gpu.draws[0].quad_count, 5);
        assert_eq!(
            gpu.drawn().iter().map(|g| g.quad_count).collect::<Vec<_>>(),
            [5],
            "what the evicted section held is given back, not left as a hole"
        );
    }

    /// Flushes the loader, handing back what it queued.
    fn flushed(loader: &mut Loader) -> Vec<Placement> {
        let uploads = Uploads::default();
        loader.flush_to(&uploads);
        std::iter::from_fn(|| uploads.pop())
            .filter_map(|upload| match upload {
                Upload::Geometry(placement) => Some(placement),
                _ => None,
            })
            .collect()
    }

    /// The terrain buffers as the GPU holds them, landed a word at a time because the upload
    /// budget may cut an upload anywhere. A placement's draw list swaps in once it has landed.
    #[derive(Default)]
    struct Gpu {
        /// The section whose mesh last wrote each quad.
        quads: HashMap<u64, [i32; 3]>,
        slots: HashMap<u32, [i32; 3]>,
        groups: HashMap<u64, Group>,
        draws: Vec<Draw>,
    }

    impl Gpu {
        /// Flushes the loader and lands what it queued. After every word it checks that no
        /// drawn record reads quads the mesh of another section wrote, then calls `check`.
        fn flush(&mut self, loader: &mut Loader, mut check: impl FnMut(&Self)) {
            for placement in flushed(loader) {
                if let Some(desc) = placement.sections.1.first() {
                    let first = placement.quads.0 / (QUAD_WORDS * 4) as u64;
                    for quad in first..first + placement.quads.1.len() as u64 {
                        self.quads.insert(quad, desc.section);
                        self.check(&mut check);
                    }
                    let slot = placement.sections.0 / size_of::<SectionDesc>() as u64;
                    self.slots.insert(slot as u32, desc.section);
                    self.check(&mut check);
                }
                for (offset, records) in placement.groups {
                    let first = offset / size_of::<Group>() as u64;
                    for (at, record) in (first..).zip(records) {
                        self.land_record(at, record, &mut check);
                    }
                }
                if let Some(draws) = placement.draws {
                    self.draws = draws;
                    self.check(&mut check);
                }
            }
        }

        fn land_record(&mut self, at: u64, record: Group, check: &mut impl FnMut(&Self)) {
            for word in 0..5 {
                let held = self.groups.entry(at).or_default();
                match word {
                    0 => held.quad_base = record.quad_base,
                    1 => held.quad_count = record.quad_count,
                    2 => held.section = record.section,
                    3 => held.face = record.face,
                    _ => held.bounds = record.bounds,
                }
                self.check(check);
            }
        }

        fn check(&self, check: &mut impl FnMut(&Self)) {
            for group in self.drawn() {
                let section = self.slots.get(&group.section);
                for quad in group.quad_base..group.quad_base + group.quad_count {
                    assert_eq!(
                        self.quads.get(&u64::from(quad)),
                        section,
                        "a drawn record reads only quads the mesh of its own section wrote"
                    );
                }
            }
            check(self);
        }

        /// The records the greedy stream draws.
        fn drawn(&self) -> Vec<Group> {
            let Some(greedy) = self.draws.first() else {
                return Vec::new();
            };
            (greedy.first_group..greedy.first_group + greedy.group_count)
                .filter_map(|index| self.groups.get(&u64::from(index)).copied())
                .filter(|group| group.quad_count != 0)
                .collect()
        }

        fn drawn_sections(&self) -> Vec<[i32; 3]> {
            self.drawn()
                .iter()
                .filter_map(|group| self.slots.get(&group.section).copied())
                .collect()
        }
    }

    fn blended_sections(loader: &Loader, stream: usize) -> Vec<[i32; 3]> {
        loader.streams[stream]
            .groups
            .iter()
            .filter(|group| group.quad_count != 0)
            .map(|group| loader.owners[group.section as usize])
            .collect()
    }

    #[test]
    fn blended_records_run_from_the_farthest_section_whatever_order_they_landed_in() {
        let blended_stream = (0..STREAMS).find(|&index| blended(index)).unwrap();
        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8, true);
        loader.look_from(Vec3::splat(8.0));

        let landed = [[1, 0, 0], [-3, 0, 0], [0, 0, 2], [3, 0, 0], [0, 1, 0]];
        for (slot, at) in landed.into_iter().enumerate() {
            let mut mesh = one_greedy_group(at, slot as u32, 1);
            mesh.spans.swap(0, blended_stream);
            loader
                .place(mesh, slot as u32, &mut cave)
                .unwrap_or_else(|_| panic!("the arena has room"));
        }
        flushed(&mut loader);
        assert_eq!(
            blended_sections(&loader, blended_stream),
            [[-3, 0, 0], [3, 0, 0], [0, 0, 2], [0, 1, 0], [1, 0, 0]],
            "farthest first, and a tie in distance goes by position"
        );

        loader.look_from(Vec3::new(4.0 * 16.0 + 8.0, 8.0, 8.0));
        flushed(&mut loader);
        assert_eq!(
            blended_sections(&loader, blended_stream),
            [[-3, 0, 0], [0, 0, 2], [0, 1, 0], [1, 0, 0], [3, 0, 0]],
            "a camera in another section orders them again"
        );

        loader.evict([0, 1, 0], &mut cave);
        flushed(&mut loader);
        assert_eq!(
            blended_sections(&loader, blended_stream),
            [[-3, 0, 0], [0, 0, 2], [1, 0, 0], [3, 0, 0]],
            "an evicted section takes its own records with it and nobody else's"
        );
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
            faces: vec![[0; mcrs_minecraft_mesh::pack::FACE_WORDS]; 4],
            complex: Vec::new(),
            groups: (0..total)
                .map(|quad| Group {
                    quad_base: quad,
                    quad_count: 1,
                    section: slot,
                    face: 0,
                    bounds: 0,
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
            groups: 1 << 13,
            sections: 1 << 10,
            upload: 0,
            tint_size: [512; 2],
        });
        let mut cave = CaveCull::new(1 << 10, true);

        // Each stream takes its new block beside the one the GPU draws, from an arena sized to
        // hold two of each, and gives the old one back behind the draw list that leaves it.
        let counts = [1u32, 3, 2];
        let placed = 342u32;
        let mut draws = Vec::new();
        for index in 0..placed {
            loader
                .place(
                    greedy_groups([index as i32, 0, 0], index, counts),
                    index,
                    &mut cave,
                )
                .unwrap_or_else(|_| panic!("the arena has room"));
            draws = flushed(&mut loader)
                .into_iter()
                .find_map(|placement| placement.draws)
                .expect("a flush hands over the whole draw list");
        }

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
        assert_eq!(sections.work(waiting), Work::Queued);
        assert!(sections.is_meshing(meshing));
        assert_eq!(sections.queued(), 1);
        assert_eq!(sections.pop_nearest([0; 3]), Some(waiting));
        assert_eq!(sections.pop_nearest([0; 3]), None);
    }

    #[test]
    fn columns_the_server_takes_back_or_completes_reach_the_loader() {
        a_section_is_queued_when_the_last_of_its_neighbours_arrives_not_when_it_does();
        a_mesh_read_out_of_a_column_the_server_took_back_is_thrown_away();
        a_column_taken_back_before_it_is_adopted_leaves_the_section_count_where_it_was();
    }

    fn a_section_is_queued_when_the_last_of_its_neighbours_arrives_not_when_it_does() {
        use mcrs_minecraft_client_world::columns::{Column, Extent, Section};

        let extent = Extent {
            min_section_y: 0,
            sections: 1,
        };
        let stone = || {
            Column::unlit(
                0,
                vec![Some(Section {
                    blocks: Box::new(
                        [mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air");
                            mcrs_minecraft_mesh::SECTION_VOLUME],
                    ),
                    biomes: PalettedContainer::Homogeneous(0),
                    states: vec![
                        mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air"),
                    ],
                })],
            )
        };

        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8, true);
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
            loader.sections.work([0, 0, 0]) == Work::Queued,
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

    /// Stone in the nine columns around the origin, so the section there is meshable, and a
    /// loader whose quad arena holds `meshes` meshes of three quads.
    fn relit_fixture(meshes: usize) -> (Loader, ColumnStore) {
        use mcrs_minecraft_client_world::columns::{Column, Extent, Section};

        let stone = mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air");
        let mut store = ColumnStore::default();
        store.enter(Extent {
            min_section_y: 0,
            sections: 1,
        });
        for x in -1..=1 {
            for z in -1..=1 {
                let section = Section {
                    blocks: Box::new([stone; mcrs_minecraft_mesh::SECTION_VOLUME]),
                    biomes: PalettedContainer::Homogeneous(0),
                    states: vec![stone],
                };
                store.insert(ColumnPos::new(x, z), Column::unlit(0, vec![Some(section)]));
            }
        }
        let loader = Loader::new(&Budget {
            quads: 4 * meshes,
            models: 1 << 12,
            faces: 1 << 16,
            groups: 1 << 12,
            sections: 1 << 8,
            upload: 0,
            tint_size: [512; 2],
        });
        (loader, store)
    }

    /// Lands a mesh of three quads for `at` the way the pool hands it over, returning its slot.
    fn land_mesh(loader: &mut Loader, at: [i32; 3], cave: &mut CaveCull) -> u32 {
        let slot = loader.take_slot().expect("a free slot");
        loader.sections.dequeue(at);
        loader.land(one_greedy_group(at, slot, 3), slot, cave);
        slot
    }

    #[test]
    fn no_frame_draws_a_hole_or_foreign_quads_where_a_relit_section_is_replaced() {
        let (mut loader, mut store) = relit_fixture(4);
        let mut cave = CaveCull::new(1 << 8, true);
        let mut gpu = Gpu::default();
        adopt(&mut loader, &mut store, &mut cave);
        let (relit, far) = ([0, 0, 0], [40, 0, 0]);
        let mut slot = land_mesh(&mut loader, relit, &mut cave);
        for at in [far, [1, 0, 0], [-1, 0, 0]] {
            land_mesh(&mut loader, at, &mut cave);
        }
        gpu.flush(&mut loader, |_| {});

        // The first relight finds the arena full and evicts the far section, whose room the
        // new mesh takes; the second finds the room the first gave back, and leaves the stream
        // dead enough to be rebuilt.
        for _ in 0..2 {
            loader.stale.insert(relit);
            loader.remesh_stale(&store, &mut cave);
            assert!(
                loader.sections.work(relit) == Work::Queued,
                "the section is meshed again"
            );
            let old = slot;
            slot = land_mesh(&mut loader, relit, &mut cave);
            gpu.flush(&mut loader, |gpu| {
                assert!(
                    gpu.drawn_sections().contains(&relit),
                    "the relit section is drawn after every word that lands"
                );
            });
            assert_eq!(
                gpu.drawn_sections()
                    .iter()
                    .filter(|&&at| at == relit)
                    .count(),
                1,
                "the new mesh takes the place of the old one"
            );
            assert!(
                loader.free_slots.contains(&old),
                "and the old mesh gives its slot back"
            );
        }
        assert!(!gpu.drawn_sections().contains(&far));
        assert_eq!(
            loader.quads.held(),
            3 * 4,
            "the old mesh gives its room back"
        );
    }

    #[test]
    fn a_relit_section_in_a_full_arena_takes_the_room_of_the_mesh_it_replaces() {
        let (mut loader, mut store) = relit_fixture(1);
        let mut cave = CaveCull::new(1 << 8, true);
        let mut gpu = Gpu::default();
        adopt(&mut loader, &mut store, &mut cave);
        let relit = [0, 0, 0];
        land_mesh(&mut loader, relit, &mut cave);
        gpu.flush(&mut loader, |_| {});

        loader.stale.insert(relit);
        loader.remesh_stale(&store, &mut cave);
        let new = land_mesh(&mut loader, relit, &mut cave);
        gpu.flush(&mut loader, |_| {});
        assert_eq!(
            loader.sections.resident(relit).map(|held| held.slot),
            Some(new),
            "the new mesh is placed rather than left waiting for room nobody gives back"
        );
        assert_eq!(gpu.drawn_sections(), [relit]);
    }

    fn a_mesh_read_out_of_a_column_the_server_took_back_is_thrown_away() {
        use mcrs_minecraft_client_world::columns::{Column, Extent, Section};

        let stone = || {
            Column::unlit(
                0,
                vec![Some(Section {
                    blocks: Box::new(
                        [mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air");
                            mcrs_minecraft_mesh::SECTION_VOLUME],
                    ),
                    biomes: PalettedContainer::Homogeneous(0),
                    states: vec![
                        mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air"),
                    ],
                })],
            )
        };

        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8, true);
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
        assert!(
            loader.sections.resident(centre).is_none(),
            "so it is not drawn"
        );
        assert_eq!(loader.free_slots, vec![slot], "and its slot goes back");
        assert!(
            loader.sections.work(centre) != Work::Queued,
            "with the column gone there is nothing to mesh again"
        );

        // The same again, only the server sends the column back before the mesh lands.
        loader.sections.start_meshing(centre);
        store.insert(ColumnPos::new(0, 0), stone());
        store.remove(ColumnPos::new(0, 0));
        store.insert(ColumnPos::new(0, 0), stone());
        adopt(&mut loader, &mut store, &mut cave);
        assert!(
            loader.sections.work(centre) != Work::Queued,
            "the arrival cannot queue a section a task still holds"
        );
        assert!(loader.discard_orphan(centre, slot, &store));
        assert!(
            loader.sections.work(centre) == Work::Queued,
            "the column is back, so the section is meshed again out of what it holds now"
        );
    }

    fn a_column_taken_back_before_it_is_adopted_leaves_the_section_count_where_it_was() {
        use mcrs_minecraft_client_world::columns::{Column, Extent, Section};

        let mut loader = loader();
        let mut cave = CaveCull::new(1 << 8, true);
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
                    blocks: Box::new(
                        [mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air");
                            mcrs_minecraft_mesh::SECTION_VOLUME],
                    ),
                    biomes: PalettedContainer::Homogeneous(0),
                    states: vec![
                        mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air"),
                    ],
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

    #[test]
    fn a_column_that_arrives_before_the_camera_is_kept_for_the_mesher() {
        let mut app = App::new();
        app.init_resource::<ColumnStore>()
            .insert_resource(loader())
            .add_message::<ColumnChange>()
            .add_systems(Update, collect_column_changes);
        let pos = ColumnPos::new(2, 3);
        app.world_mut().write_message(ColumnChange::Arrived(
            pos,
            Arc::new(mcrs_minecraft_client_world::columns::Column::unlit(
                0,
                Vec::new(),
            )),
        ));
        for _ in 0..3 {
            app.update();
        }

        let changes = &app.world().resource::<Loader>().changes;
        assert_eq!(changes.len(), 1);
        assert!(matches!(changes[0], ColumnChange::Arrived(at, _) if at == pos));
    }

    mod exhaustive {
        use super::*;

        #[test]
        fn a_stream_the_arena_cannot_rebuild_beside_its_block_stops_sending_itself_again() {
            let mut loader = Loader::new(&Budget {
                quads: 1 << 13,
                models: 1 << 12,
                faces: 1 << 16,
                groups: 1 << 12,
                sections: 1 << 13,
                upload: 0,
                tint_size: [512; 2],
            });
            let mut cave = CaveCull::new(1 << 13, true);

            // Past 2048 records the stream wants the whole arena, which it cannot take while
            // the GPU still draws the block it has.
            let mut last = Vec::new();
            for index in 0..3000u32 {
                loader
                    .place(
                        one_greedy_group([index as i32, 0, 0], index, 1),
                        index,
                        &mut cave,
                    )
                    .unwrap_or_else(|_| panic!("the arena has room"));
                last = flushed(&mut loader);
            }

            let draws = last
                .iter()
                .find_map(|placement| placement.draws.as_ref())
                .expect("a flush hands over the whole draw list");
            assert_eq!(
                draws[0].group_count,
                1 << 11,
                "a stream past what the arena can give it draws what its block holds"
            );
            assert!(
                last.iter().all(|placement| placement.groups.is_empty()),
                "and stops packing and sending itself again for the room that is not coming"
            );
        }
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
        let Sections { states, queue, .. } = &mut sections;
        queue.rebucket([0; 3], |at| {
            states
                .get(&at)
                .is_some_and(|section| section.work == Work::Queued)
        });
        assert_eq!(sections.pop_nearest([0; 3]), Some(requeued));
        assert_eq!(sections.pop_nearest([0; 3]), Some(other));
        assert_eq!(sections.pop_nearest([0; 3]), None);
    }
}
