use std::collections::BinaryHeap;
use std::sync::Arc;

use bevy::ecs::system::SystemParam;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures::check_ready};
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_client_world::camera_section_ready::CameraSectionReady;
use mcrs_minecraft_client_world::columns::{
    ClientTerrainSet, ColumnChange, ColumnStore, Columns, Extent,
};
#[cfg(feature = "dev")]
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, TraceEvent};
use mcrs_minecraft_mesh::SECTION_SIZE;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::{BlockPos, ColumnPos, LocalPos, SectionPos};

use crate::cave::{self, CaveCull, NO_SLOT};
use crate::section_drawn::SectionDrawn;
use crate::stream_focus::StreamFocus;
use crate::tint::{self, BiomeTints};
use crate::vanilla::VanillaAssets;
use mcrs_minecraft_client_world::joined_game::CurrentDimension;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::registries::SessionSet;
use mcrs_minecraft_client_world::session::session_boundary;
use mcrs_minecraft_dimension::CardinalLight;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_mesh::arena::{Arena, Block};
use mcrs_minecraft_mesh::block::BlockInfo;
use mcrs_minecraft_mesh::pack::QUAD_WORDS;
use mcrs_minecraft_mesh::smooth_light::SmoothLight;
use mcrs_minecraft_mesh::{Draw, DrawKind, Group, QuadFormat, Scratch, SectionMesh};
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
            .init_resource::<StreamFocus>()
            .add_message::<SectionDrawn>()
            .insert_resource(BlockCatalog::new())
            .insert_resource(ColumnTints::new(self.budget.tint_size))
            .insert_resource(self.uploads.clone())
            .insert_resource(self.pacing)
            .init_resource::<LightOptions>()
            .init_resource::<TerrainShade>()
            .init_resource::<CameraSectionReady>()
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
                    bake_catalog.run_if(in_state(VanillaAssets::Ready)),
                    tint_columns,
                    place_meshes,
                    project_camera_section_ready,
                    flush_streams,
                    admit_meshing,
                )
                    .chain()
                    .in_set(ClientTerrainSet::Build)
                    .run_if(can_stream),
            )
            .add_systems(
                PostUpdate,
                cave::record_drawn_sections
                    .before(cave::cave_cull)
                    .run_if(resource_exists::<CaveCull>),
            )
            .add_systems(
                Update,
                clear_biome_tints
                    .in_set(SessionSet::Clear)
                    .run_if(session_boundary),
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
                adopt_columns
                    .in_set(ClientTerrainSet::Build)
                    .after(remesh_on_light_options)
                    .before(bake_catalog)
                    .run_if(resource_exists::<Blocks>)
                    .run_if(resource_exists::<ColumnStore>),
            );
    }
}

type MeshingSection = ([i32; 3], u32, Task<(SectionMesh, Scratch)>);
type FarthestResidents = ([i32; 3], BinaryHeap<(u32, [i32; 3])>);

#[derive(Resource)]
pub struct Loader {
    columns: usize,
    sections_total: usize,
    sections: Sections,
    stale: HashSet<[i32; 3]>,
    geometry: Vec<Placement>,
    replaced: Vec<Resident>,
    retired_blocks: Vec<Block>,
    meshing: Vec<MeshingSection>,
    scratches: Vec<Scratch>,
    buckets: [DrawBucket; DrawKind::COUNT],
    owners: Vec<[i32; 3]>,
    slots: usize,
    free_slots: Vec<u32>,
    slots_used: u32,
    dirty: bool,
    anchor: Vec3,
    evicted: usize,
    farthest: Option<FarthestResidents>,
    quads: Arena,
    models: Arena,
    faces: Arena,
    groups: Arena,
    #[cfg(feature = "dev")]
    pub(crate) trace: Vec<TraceEvent>,
}

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
    groups: [(u32, u32); DrawKind::COUNT],
}

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
    Meshing {
        orphaned: bool,
    },
    Deferred,
}

struct DrawBucket {
    groups: Vec<Group>,
    block: Block,
    quads_end: u32,
    dead: usize,
    touched: Vec<(u32, u32)>,
    rebuild: bool,
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
            columns: 0,
            sections_total: 0,
            sections: Sections::default(),
            stale: HashSet::new(),
            geometry: Vec::new(),
            replaced: Vec::new(),
            retired_blocks: Vec::new(),
            meshing: Vec::new(),
            scratches: Vec::new(),
            buckets: std::array::from_fn(|_| DrawBucket::new()),
            owners: vec![[0; 3]; budget.sections],
            slots: budget.sections,
            free_slots: Vec::new(),
            slots_used: 0,
            dirty: false,
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

    fn surrounded(&self, pos: ColumnPos, store: &ColumnStore) -> bool {
        (-1..=1).all(|dz| (-1..=1).all(|dx| store.holds(ColumnPos::new(pos.x + dx, pos.z + dz))))
    }

    fn meshable(&self, at: [i32; 3], store: &ColumnStore) -> bool {
        !self.sections.is_meshing(at)
            && store
                .stored_section(SectionPos::new(at[0], at[1], at[2]))
                .is_some()
            && self.surrounded(ColumnPos::new(at[0], at[2]), store)
    }

    fn camera_section_drawable(&self, store: &ColumnStore, focus: &StreamFocus) -> bool {
        let at = focus.section();
        let Some(extent) = store.extent() else {
            return false;
        };
        let sections = i32::try_from(extent.sections).unwrap_or(i32::MAX);
        if !(extent.min_section_y..extent.min_section_y.saturating_add(sections)).contains(&at[1]) {
            return true;
        }
        if !self.surrounded(ColumnPos::new(at[0], at[2]), store) {
            return false;
        }
        self.sections.resident(at).is_some()
            || (self.sections.work(at) == Work::Idle
                && store
                    .stored_section(SectionPos::new(at[0], at[1], at[2]))
                    .is_none())
    }

    fn rebuild_blended(&mut self) {
        for (index, bucket) in self.buckets.iter_mut().enumerate() {
            if blended(index) && bucket.live() != 0 {
                bucket.rebuild = true;
                self.dirty = true;
            }
        }
    }

    fn enqueue(&mut self, at: [i32; 3], focus: &StreamFocus) {
        self.sections.enqueue(at, focus.section());
    }

    fn enqueue_around(
        &mut self,
        pos: ColumnPos,
        extent: Extent,
        store: &ColumnStore,
        focus: &StreamFocus,
    ) {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let column = ColumnPos::new(pos.x + dx, pos.z + dz);
                if !self.surrounded(column, store) {
                    continue;
                }
                for step in 0..extent.sections {
                    let at = [column.x, extent.min_section_y + step as i32, column.z];
                    if self.sections.resident(at).is_none() && self.meshable(at, store) {
                        self.enqueue(at, focus);
                    }
                }
            }
        }
    }

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

    fn remesh_stale(
        &mut self,
        store: &ColumnStore,
        focus: &StreamFocus,
        drawn: &mut Messages<SectionDrawn>,
    ) {
        for at in std::mem::take(&mut self.stale) {
            if self.sections.is_meshing(at) {
                self.stale.insert(at);
            } else if self.meshable(at, store) {
                self.enqueue(at, focus);
            } else {
                self.evict(at, drawn);
            }
        }
    }

    fn discard_orphan(
        &mut self,
        at: [i32; 3],
        slot: u32,
        store: &ColumnStore,
        focus: &StreamFocus,
    ) -> bool {
        if self.sections.finish_meshing(at) != Some(true) {
            return false;
        }
        self.free_slots.push(slot);
        if self.meshable(at, store) {
            self.enqueue(at, focus);
        }
        true
    }

    fn requeue_deferred(&mut self, focus: &StreamFocus) {
        self.sections.requeue_deferred(focus.section());
    }

    fn take_wanted(
        &mut self,
        want: usize,
        store: &ColumnStore,
        focus: &StreamFocus,
    ) -> Vec<[i32; 3]> {
        if want == 0 || self.sections.queued() == 0 {
            return Vec::new();
        }
        let camera = focus.section();
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

    fn victim(&mut self, candidate: f32, focus: &StreamFocus) -> Option<[i32; 3]> {
        let camera = focus.section();
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
        let eye = focus.eye();
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

    fn evict(&mut self, section: [i32; 3], drawn: &mut Messages<SectionDrawn>) {
        if let Some(held) = self.sections.take_resident(section) {
            self.release(section, held, drawn);
        }
    }

    fn release(&mut self, section: [i32; 3], held: Resident, drawn: &mut Messages<SectionDrawn>) {
        if held.slot != NO_SLOT {
            self.doom(&held);
            for (bucket, &(first, count)) in self.buckets.iter_mut().zip(&held.groups) {
                kill(bucket, first, count);
                bucket.touched.push((first, first + count));
            }
            self.free_slots.push(held.slot);
        }
        self.free_room(&held);
        drawn.write(SectionDrawn::Forgotten { section });
        self.evicted += 1;
    }

    fn doom(&mut self, held: &Resident) {
        for (bucket, &(_, count)) in self.buckets.iter_mut().zip(&held.groups) {
            bucket.dead += count as usize;
            if count != 0 && bucket.dead * 2 >= bucket.groups.len() {
                bucket.rebuild = true;
            }
        }
        self.dirty = true;
    }

    fn free_room(&mut self, held: &Resident) {
        self.quads.free(held.quads);
        self.models.free(held.models);
        self.faces.free(held.faces);
    }

    #[expect(
        clippy::excessive_nesting,
        reason = "diagonal neighbours stay beside the arrival that stales them"
    )]
    fn adopt<'a>(
        &mut self,
        changes: impl IntoIterator<Item = &'a ColumnChange>,
        store: &ColumnStore,
        catalog: &mut BlockCatalog,
        tints: &mut ColumnTints,
        focus: &StreamFocus,
        drawn: &mut Messages<SectionDrawn>,
    ) {
        let extent = store.extent();
        for change in changes {
            match change {
                &ColumnChange::Departed(pos, ref column) => {
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
                        self.evict(at, drawn);
                    }
                }
                &ColumnChange::Arrived(pos, ref column) => {
                    #[cfg(feature = "dev")]
                    self.trace
                        .push(TraceEvent::mark(pos, ColumnStage::Received));
                    self.columns += 1;
                    tints.to_tint.push(pos);
                    for (_, section) in column.sections() {
                        let Some(section) = section else { continue };
                        self.sections_total += 1;
                        for &state in &section.states {
                            catalog.owe(state.number());
                        }
                    }
                    if let Some(extent) = extent {
                        self.enqueue_around(pos, extent, store, focus);
                    }
                }
                ColumnChange::Filled(_) => self.sections_total += 1,
                ColumnChange::Signed(_) => {}
                &ColumnChange::Reblocked(at, state) => {
                    catalog.owe(state.number());
                    self.mark_stale_around(at);
                }
                &ColumnChange::Relit(pos, ref rows) => {
                    for &sy in rows {
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
    }

    fn reserve(&mut self, mesh: &SectionMesh) -> Option<[Block; 3]> {
        let quads = self.quads.alloc(mesh.greedy.len());
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
        drawn: &mut Messages<SectionDrawn>,
    ) -> Result<(), Box<SectionMesh>> {
        let mut first = 0usize;
        let mut ends = [0u32; DrawKind::COUNT];
        for ((bucket, draw), end) in self.buckets.iter().zip(&mesh.draws).zip(&mut ends) {
            let run = draw.group_count as usize;
            let Some(groups) = mesh.groups.get(first..first + run) else {
                return Err(Box::new(mesh));
            };
            *end = bucket.quads_end;
            for group in groups {
                let Some(next) = end.checked_add(group.quad_count) else {
                    return Err(Box::new(mesh));
                };
                *end = next;
            }
            first += run;
        }

        let Some([quads, models, faces]) = self.reserve(&mesh) else {
            return Err(Box::new(mesh));
        };

        let mut placed = mesh.groups;
        let mut first = 0usize;
        let mut ranges = [(0u32, 0u32); DrawKind::COUNT];
        for (((index, bucket), kind), end) in self
            .buckets
            .iter_mut()
            .enumerate()
            .zip(DrawKind::ALL)
            .zip(ends)
        {
            let run = mesh.draws[index].group_count as usize;
            let base = match kind.format {
                QuadFormat::Greedy => quads.offset,
                QuadFormat::Model => models.offset,
            } as u32;
            bucket.quads_end = end;
            for group in &mut placed[first..first + run] {
                group.quad_base += base;
            }
            if run != 0 {
                let at = bucket.groups.len() as u32;
                bucket.groups.extend_from_slice(&placed[first..first + run]);
                ranges[index] = (at, run as u32);
                bucket.touched.push((at, at + run as u32));
                if kind.translucent() || bucket.groups.len() > bucket.block.capacity() {
                    bucket.rebuild = true;
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
                quads: ((quads.offset * QUAD_WORDS * 4) as u64, mesh.greedy),
                vertices: ((models.offset * 4 * 3 * 4) as u64, mesh.models),
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
        drawn.write(SectionDrawn::Laid {
            section,
            slot,
            connectivity: mesh.connectivity,
        });
        #[cfg(feature = "dev")]
        self.trace.push(TraceEvent::mark(
            ColumnPos::new(section[0], section[2]),
            ColumnStage::Meshed,
        ));
        self.settle(
            section,
            Resident {
                quads,
                models,
                faces,
                slot,
                groups: ranges,
            },
        );
        Ok(())
    }

    fn settle(&mut self, section: [i32; 3], held: Resident) {
        let Some(replaced) = self.sections.settle(section, held) else {
            return;
        };
        if replaced.slot == NO_SLOT {
            self.free_room(&replaced);
        } else {
            self.doom(&replaced);
            self.replaced.push(replaced);
        }
    }

    fn touched_runs(&mut self) -> Vec<(u64, Vec<Group>)> {
        let mut runs = Vec::new();
        for bucket in &mut self.buckets {
            let capacity = bucket.block.capacity() as u32;
            let mut ranges = std::mem::take(&mut bucket.touched);
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
                bucket.write(first, end, &mut runs);
            }
            bucket.touched = ranges;
        }
        runs
    }

    fn draws(&self) -> Vec<Draw> {
        self.buckets
            .iter()
            .zip(DrawKind::ALL)
            .map(|(bucket, kind)| Draw {
                kind,
                first_group: bucket.block.offset as u32,
                group_count: bucket.groups.len().min(bucket.block.capacity()) as u32,
                quad_count: bucket.quads_end,
            })
            .collect()
    }

    fn flush_to(&mut self, uploads: &Uploads, focus: &StreamFocus) {
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
        self.rebuild_blocks(focus);
        uploads.push(Upload::Geometry(Placement {
            groups: self.touched_runs(),
            draws: Some(self.draws()),
            ..Placement::default()
        }));
        let mut replaced = std::mem::take(&mut self.replaced);
        for held in replaced.drain(..) {
            for (bucket, &(first, count)) in self.buckets.iter_mut().zip(&held.groups) {
                kill(bucket, first, count);
                bucket.touched.push((first, first + count));
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

    fn land(
        &mut self,
        mesh: SectionMesh,
        slot: u32,
        focus: &StreamFocus,
        drawn: &mut Messages<SectionDrawn>,
    ) {
        let at = mesh.section;
        let here = distance_from(focus.eye(), at);
        let mut pending = mesh;
        loop {
            let back = match self.place(pending, slot, drawn) {
                Ok(()) => {
                    self.placed(at);
                    return;
                }
                Err(back) => back,
            };
            pending = *back;
            if let Some(victim) = self.victim(here, focus) {
                self.evict(victim, drawn);
                self.sections.defer(victim);
            } else if let Some(own) = self.sections.take_resident(at) {
                self.release(at, own, drawn);
            } else {
                self.sections.defer(at);
                self.free_slots.push(slot);
                return;
            }
        }
    }

    fn rebuild_blocks(&mut self, focus: &StreamFocus) {
        for index in 0..DrawKind::COUNT {
            let bucket = &mut self.buckets[index];
            if !bucket.rebuild {
                continue;
            }
            let Some(block) = self.groups.alloc(bucket.live().max(MIN_BLOCK)) else {
                if !bucket.starved {
                    bucket.starved = true;
                    warn!(
                        draw = index,
                        live = bucket.live(),
                        held = self.groups.held(),
                        capacity = self.groups.capacity(),
                        "the group arena has no room to rebuild this draw bucket; it draws only what \
                         its block holds and the rest of its sections stay off the screen"
                    );
                }
                continue;
            };
            bucket.starved = false;
            self.retired_blocks
                .push(std::mem::replace(&mut bucket.block, block));
            for held in &mut self.replaced {
                let (first, count) = std::mem::take(&mut held.groups[index]);
                kill(bucket, first, count);
            }
            self.pack_bucket(index, focus);
        }
    }

    fn pack_bucket(&mut self, index: usize, focus: &StreamFocus) {
        let camera = IVec3::from_array(focus.section());
        let Self {
            buckets,
            owners,
            sections,
            ..
        } = self;
        let bucket = &mut buckets[index];
        let live = bucket.live();
        let mut packed: Vec<Group> = std::mem::take(&mut bucket.groups)
            .into_iter()
            .filter(|group| group.quad_count != 0)
            .collect();
        if blended(index) {
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
            quads = quads.saturating_add(group.quad_count);
        }
        bucket.groups = packed;
        bucket.quads_end = quads;
        bucket.dead = 0;
        bucket.touched.clear();
        bucket.touched.push((0, live as u32));
        bucket.rebuild = false;
    }
}

impl DrawBucket {
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

    fn live(&self) -> usize {
        self.groups.len() - self.dead
    }

    fn write(&self, first: u32, end: u32, runs: &mut Vec<(u64, Vec<Group>)>) {
        let Some(records) = self.groups.get(first as usize..end as usize) else {
            return;
        };
        let offset = (self.block.offset + first as usize) * size_of::<Group>();
        runs.push((offset as u64, records.to_vec()));
    }
}

fn kill(bucket: &mut DrawBucket, first: u32, count: u32) {
    if let Some(groups) = bucket
        .groups
        .get_mut(first as usize..first as usize + count as usize)
    {
        for group in groups {
            group.quad_count = 0;
        }
    }
}

pub fn load_gui_atlas(catalog: Res<BlockCatalog>, mut commands: Commands) {
    if let Some(pack) = catalog.pack()
        && let Err(reason) =
            mcrs_minecraft_client_gui::scene::load_gui_atlas(pack, commands.reborrow())
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

    fn corner(&self, pos: ColumnPos) -> [u32; 2] {
        let wrapped = |axis: i32, size: u32| (axis * SECTION_SIZE as i32).rem_euclid(size as i32);
        [
            wrapped(pos.x, self.size[0]) as u32,
            wrapped(pos.z, self.size[1]) as u32,
        ]
    }
}

const RUN_GAP: u32 = 64;
const MIN_BLOCK: usize = 256;

fn blended(index: usize) -> bool {
    DrawKind::from_index(index).is_some_and(DrawKind::translucent)
}

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

    fn finish_meshing(&mut self, at: [i32; 3]) -> Option<bool> {
        let Work::Meshing { orphaned } = self.work(at) else {
            return None;
        };
        self.set_work(at, Work::Idle);
        Some(orphaned)
    }

    fn settle(&mut self, at: [i32; 3], held: Resident) -> Option<Resident> {
        let replaced = self.states.entry(at).or_default().resident.replace(held);
        if replaced.is_none() {
            self.tally.residents += 1;
        }
        replaced
    }

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

#[derive(Default)]
struct MeshQueue {
    buckets: Vec<Vec<[i32; 3]>>,
    reference: [i32; 3],
    entries: usize,
    scratch: Vec<[i32; 3]>,
}

const QUEUE_BUCKETS: usize = 256;
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

fn heap_key(camera_section: [i32; 3], section: [i32; 3]) -> u32 {
    let middle = (Vec3::from_array(camera_section.map(|n| n as f32)) + 0.5) * SECTION_SIZE as f32;
    distance_from(middle, section).to_bits()
}

fn distance_from(camera: Vec3, section: [i32; 3]) -> f32 {
    let min = Vec3::from_array(section.map(|n| (n * SECTION_SIZE as i32) as f32));
    let max = min + Vec3::splat(SECTION_SIZE as f32);
    (camera.clamp(min, max) - camera).length()
}

pub(crate) fn can_stream(
    store: Option<Res<ColumnStore>>,
    blocks: Option<Res<Blocks>>,
    cameras: Query<(), With<Camera3d>>,
) -> bool {
    store.is_some() && blocks.is_some() && cameras.single().is_ok()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the queue buckets are allocated once, and the eviction heap is rebuilt only when the camera enters another section"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn follow_camera(
    mut loader: ResMut<Loader>,
    mut focus: ResMut<StreamFocus>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let next = refocus(&mut loader, *focus, camera.translation());
    focus.set_if_neq(next);
}

fn refocus(loader: &mut Loader, mut focus: StreamFocus, eye: Vec3) -> StreamFocus {
    if focus.look_from(eye) {
        loader.rebuild_blended();
    }
    if eye.distance(loader.anchor) > HYSTERESIS {
        loader.anchor = eye;
        loader.requeue_deferred(&focus);
    }
    focus
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only on a frame with column changes, and the early return skips the other frames"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn adopt_columns(
    mut changes: MessageReader<ColumnChange>,
    mut loader: ResMut<Loader>,
    mut catalog: ResMut<BlockCatalog>,
    mut tints: ResMut<ColumnTints>,
    mut drawn: ResMut<Messages<SectionDrawn>>,
    focus: Res<StreamFocus>,
    store: Res<ColumnStore>,
    definitions: Res<Blocks>,
) {
    if changes.is_empty() {
        return;
    }
    let _adopting = info_span!("stream adopt").entered();
    catalog.cover(&definitions);
    loader.adopt(
        changes.read(),
        &store,
        &mut catalog,
        &mut tints,
        &focus,
        &mut drawn,
    );
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only on the frame that starts a bake for new block states or publishes its result"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(2))]
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the tints are derived only after the registries change"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(3))]
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

fn clear_biome_tints(mut commands: Commands, mut tints: ResMut<ColumnTints>) {
    commands.remove_resource::<BiomeTints>();
    tints.to_tint.clear();
    tints.tinting.clear();
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

type Landed = ([i32; 3], u32, (SectionMesh, Scratch));

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only when meshes land or sections go stale, and the geometry moves into the upload queue"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(6))]
fn place_meshes(
    mut loader: ResMut<Loader>,
    mut drawn: ResMut<Messages<SectionDrawn>>,
    focus: Res<StreamFocus>,
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
        if loader.discard_orphan(at, slot, &store, &focus) {
            continue;
        }
        loader.land(mesh, slot, &focus, &mut drawn);
    }

    loader.remesh_stale(&store, &focus, &mut drawn);
}

fn project_camera_section_ready(
    loader: Res<Loader>,
    focus: Res<StreamFocus>,
    store: Res<ColumnStore>,
    modes: Query<&PlayerGameMode, With<Player>>,
    mut ready: ResMut<CameraSectionReady>,
) {
    let spectator = modes.iter().any(|mode| mode.0 == GameMode::Spectator);
    ready.set_if_neq(CameraSectionReady(
        spectator || loader.camera_section_drawable(&store, &focus),
    ));
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only when the geometry is dirty, and the runs move into the upload queue of the render world"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(9))]
pub(crate) fn flush_streams(
    mut loader: ResMut<Loader>,
    focus: Res<StreamFocus>,
    uploads: Res<Uploads>,
) {
    let _flushing = info_span!("stream flush").entered();
    loader.flush_to(&uploads, &focus);
}

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
    loader.stale.extend(drawn.chain(meshing));
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates only while sections wait to mesh, within the per-frame mesh budget"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(3))]
fn admit_meshing(
    mut loader: ResMut<Loader>,
    catalog: Res<BlockCatalog>,
    store: Res<ColumnStore>,
    pacing: Res<MeshPacing>,
    options: Res<LightOptions>,
    shade: Res<TerrainShade>,
    focus: Res<StreamFocus>,
) {
    if !catalog.caught_up() {
        return;
    }
    let _admitting = info_span!("stream admit").entered();
    let loader = &mut *loader;
    let pool = AsyncComputeTaskPool::get();
    let room = pacing.in_flight.saturating_sub(loader.meshing.len());
    let wanted = loader.take_wanted(room.min(pacing.per_frame), &store, &focus);
    for (taken, &at) in wanted.iter().enumerate() {
        let Some(slot) = loader.take_slot() else {
            for &back in &wanted[taken..] {
                loader.enqueue(back, &focus);
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
                let mesh = mcrs_minecraft_mesh::mesh_section(
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
    use mcrs_minecraft_mesh::SectionDraw;
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
        app.add_message::<mcrs_minecraft_network::client::ConnectionEnded>()
            .add_systems(Update, clear_biome_tints.run_if(session_boundary));
        app.insert_resource(BiomeTints::default())
            .insert_resource(ColumnTints::new([512; 2]));
        let connection = app.world_mut().spawn(ConnectionState::Game).id();
        app.update();
        assert!(app.world().get_resource::<BiomeTints>().is_some());

        app.world_mut()
            .resource_mut::<ColumnTints>()
            .to_tint
            .push(ColumnPos::new(0, 0));
        app.world_mut()
            .entity_mut(connection)
            .insert(ConnectionState::Configuration);
        app.update();
        assert!(
            app.world().get_resource::<BiomeTints>().is_none(),
            "a new registry set renumbers the biomes the table is indexed by"
        );
        assert!(
            app.world().resource::<ColumnTints>().to_tint.is_empty(),
            "a column queued in the previous world is not tinted with the next palette"
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

    fn budget() -> Budget {
        Budget {
            quads: 1 << 12,
            models: 1 << 12,
            faces: 1 << 16,
            groups: 1 << 12,
            sections: 1 << 8,
            upload: 0,
            tint_size: [512; 2],
        }
    }

    fn loader() -> Loader {
        Loader::new(&budget())
    }

    fn adopt(loader: &mut Loader, store: &mut ColumnStore, drawn: &mut Messages<SectionDrawn>) {
        let mut catalog = BlockCatalog::new();
        let mut tints = ColumnTints::new([512; 2]);
        let mut changes = Vec::new();
        store.drain_changes(&mut changes);
        catalog.cover(&mcrs_minecraft_engine_registries::test_corpus().0.0);
        loader.adopt(
            &changes,
            store,
            &mut catalog,
            &mut tints,
            &StreamFocus::default(),
            drawn,
        );
    }

    fn one_greedy_group(section: [i32; 3], slot: u32, quads: u32) -> SectionMesh {
        let mut draws = [Default::default(); DrawKind::COUNT];
        draws[0] = SectionDraw {
            group_count: 1,
            quad_count: quads,
        };
        SectionMesh {
            section,
            greedy: vec![[0; QUAD_WORDS]; quads as usize],
            faces: vec![[0; mcrs_minecraft_mesh::pack::FACE_WORDS]; 4],
            models: Vec::new(),
            groups: vec![Group {
                quad_base: 0,
                quad_count: quads,
                section: slot,
                face: 0,
                bounds: 0,
            }],
            draws,
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
        let mut drawn = Messages::<SectionDrawn>::default();
        let mut gpu = Gpu::default();

        for (slot, (at, quads)) in [([0, 0, 0], 3), ([1, 0, 0], 5)].into_iter().enumerate() {
            loader
                .place(
                    one_greedy_group(at, slot as u32, quads),
                    slot as u32,
                    &mut drawn,
                )
                .unwrap_or_else(|_| panic!("the arena has room"));
        }
        gpu.flush(&mut loader, |_| {});
        assert_eq!(
            gpu.draws.len(),
            DrawKind::COUNT,
            "one draw a bucket, however many sections"
        );
        assert_eq!(gpu.draws[0].quad_count, 8);
        assert_eq!(
            gpu.drawn().iter().map(|g| g.quad_count).collect::<Vec<_>>(),
            [3, 5],
            "the blend order of a bucket runs across the sections in it"
        );

        loader.evict([0, 0, 0], &mut drawn);
        gpu.flush(&mut loader, |_| {});
        assert_eq!(gpu.draws[0].group_count, 1);
        assert_eq!(gpu.draws[0].quad_count, 5);
        assert_eq!(
            gpu.drawn().iter().map(|g| g.quad_count).collect::<Vec<_>>(),
            [5],
            "what the evicted section held is given back, not left as a hole"
        );
    }

    fn flushed(loader: &mut Loader, focus: &StreamFocus) -> Vec<Placement> {
        let uploads = Uploads::default();
        loader.flush_to(&uploads, focus);
        std::iter::from_fn(|| uploads.pop())
            .filter_map(|upload| match upload {
                Upload::Geometry(placement) => Some(placement),
                _ => None,
            })
            .collect()
    }

    #[derive(Default)]
    struct Gpu {
        quads: HashMap<u64, [i32; 3]>,
        slots: HashMap<u32, [i32; 3]>,
        groups: HashMap<u64, Group>,
        draws: Vec<Draw>,
    }

    impl Gpu {
        fn flush(&mut self, loader: &mut Loader, mut check: impl FnMut(&Self)) {
            for placement in flushed(loader, &StreamFocus::default()) {
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

    fn blended_sections(loader: &Loader, bucket: usize) -> Vec<[i32; 3]> {
        loader.buckets[bucket]
            .groups
            .iter()
            .filter(|group| group.quad_count != 0)
            .map(|group| loader.owners[group.section as usize])
            .collect()
    }

    #[test]
    fn blended_records_run_from_the_farthest_section_whatever_order_they_landed_in() {
        let blended_bucket = (0..DrawKind::COUNT).find(|&index| blended(index)).unwrap();
        let mut loader = loader();
        let mut drawn = Messages::<SectionDrawn>::default();
        let mut focus = refocus(&mut loader, StreamFocus::default(), Vec3::splat(8.0));

        let landed = [[1, 0, 0], [-3, 0, 0], [0, 0, 2], [3, 0, 0], [0, 1, 0]];
        for (slot, at) in landed.into_iter().enumerate() {
            let mut mesh = one_greedy_group(at, slot as u32, 1);
            mesh.draws.swap(0, blended_bucket);
            loader
                .place(mesh, slot as u32, &mut drawn)
                .unwrap_or_else(|_| panic!("the arena has room"));
        }
        flushed(&mut loader, &focus);
        assert_eq!(
            blended_sections(&loader, blended_bucket),
            [[-3, 0, 0], [3, 0, 0], [0, 0, 2], [0, 1, 0], [1, 0, 0]],
            "farthest first, and a tie in distance goes by position"
        );

        focus = refocus(&mut loader, focus, Vec3::new(4.0 * 16.0 + 8.0, 8.0, 8.0));
        flushed(&mut loader, &focus);
        assert_eq!(
            blended_sections(&loader, blended_bucket),
            [[-3, 0, 0], [0, 0, 2], [0, 1, 0], [1, 0, 0], [3, 0, 0]],
            "a camera in another section orders them again"
        );

        loader.evict([0, 1, 0], &mut drawn);
        flushed(&mut loader, &focus);
        assert_eq!(
            blended_sections(&loader, blended_bucket),
            [[-3, 0, 0], [0, 0, 2], [1, 0, 0], [3, 0, 0]],
            "an evicted section takes its own records with it and nobody else's"
        );
    }

    fn greedy_groups(section: [i32; 3], slot: u32, counts: [u32; 3]) -> SectionMesh {
        let mut draws = [SectionDraw::default(); DrawKind::COUNT];
        for (index, count) in counts.iter().enumerate() {
            draws[index * 2] = SectionDraw {
                group_count: *count,
                quad_count: *count,
            };
        }
        let total = counts.iter().sum::<u32>();
        SectionMesh {
            section,
            greedy: vec![[0; QUAD_WORDS]; total as usize],
            faces: vec![[0; mcrs_minecraft_mesh::pack::FACE_WORDS]; 4],
            models: Vec::new(),
            groups: (0..total)
                .map(|quad| Group {
                    quad_base: quad,
                    quad_count: 1,
                    section: slot,
                    face: 0,
                    bounds: 0,
                })
                .collect(),
            draws,
            connectivity: mcrs_minecraft_mesh::OPEN,
        }
    }

    #[test]
    fn buckets_growing_together_reach_every_record_the_arena_has_room_for() {
        let mut loader = Loader::new(&Budget {
            quads: 1 << 12,
            models: 1 << 12,
            faces: 1 << 14,
            groups: 1 << 13,
            sections: 1 << 10,
            upload: 0,
            tint_size: [512; 2],
        });
        let mut drawn = Messages::<SectionDrawn>::default();

        let counts = [1u32, 3, 2];
        let placed = 342u32;
        let mut draws = Vec::new();
        for index in 0..placed {
            loader
                .place(
                    greedy_groups([index as i32, 0, 0], index, counts),
                    index,
                    &mut drawn,
                )
                .unwrap_or_else(|_| panic!("the arena has room"));
            draws = flushed(&mut loader, &StreamFocus::default())
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
        let extent = Extent {
            min_section_y: 0,
            sections: 1,
        };
        let stone = || stone_column(1);

        let mut loader = loader();
        let mut drawn = Messages::<SectionDrawn>::default();
        let mut store = ColumnStore::default();
        store.enter(extent);

        let mut order: Vec<ColumnPos> = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| ColumnPos::new(x, z)))
            .collect();
        let last = order.pop().expect("nine columns");
        for pos in &order {
            store.insert(*pos, stone());
        }
        adopt(&mut loader, &mut store, &mut drawn);
        assert_eq!(
            loader.sections.queued(),
            0,
            "no column is surrounded yet, so nothing is meshable"
        );

        store.insert(last, stone());
        adopt(&mut loader, &mut store, &mut drawn);
        assert!(
            loader.sections.work([0, 0, 0]) == Work::Queued,
            "the centre column arrived first; only its last neighbour makes it meshable"
        );
        assert_ne!(
            last,
            ColumnPos::new(0, 0),
            "the arrival that unblocked the centre was a neighbour, not the centre itself"
        );

        assert_eq!(
            loader.take_wanted(8, &store, &StreamFocus::default()),
            vec![[0, 0, 0]]
        );
        assert!(
            loader
                .take_wanted(8, &store, &StreamFocus::default())
                .is_empty(),
            "a section handed out once is not handed out again"
        );
    }

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

    fn land_mesh(loader: &mut Loader, at: [i32; 3], drawn: &mut Messages<SectionDrawn>) -> u32 {
        let slot = loader.take_slot().expect("a free slot");
        loader.sections.dequeue(at);
        loader.land(
            one_greedy_group(at, slot, 3),
            slot,
            &StreamFocus::default(),
            drawn,
        );
        slot
    }

    #[test]
    fn no_frame_draws_a_hole_or_foreign_quads_where_a_relit_section_is_replaced() {
        let (mut loader, mut store) = relit_fixture(4);
        let mut drawn = Messages::<SectionDrawn>::default();
        let mut gpu = Gpu::default();
        adopt(&mut loader, &mut store, &mut drawn);
        let (relit, far) = ([0, 0, 0], [40, 0, 0]);
        let mut slot = land_mesh(&mut loader, relit, &mut drawn);
        for at in [far, [1, 0, 0], [-1, 0, 0]] {
            land_mesh(&mut loader, at, &mut drawn);
        }
        gpu.flush(&mut loader, |_| {});

        for _ in 0..2 {
            loader.stale.insert(relit);
            loader.remesh_stale(&store, &StreamFocus::default(), &mut drawn);
            assert!(
                loader.sections.work(relit) == Work::Queued,
                "the section is meshed again"
            );
            let old = slot;
            slot = land_mesh(&mut loader, relit, &mut drawn);
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
        let mut drawn = Messages::<SectionDrawn>::default();
        let mut gpu = Gpu::default();
        adopt(&mut loader, &mut store, &mut drawn);
        let relit = [0, 0, 0];
        land_mesh(&mut loader, relit, &mut drawn);
        gpu.flush(&mut loader, |_| {});

        loader.stale.insert(relit);
        loader.remesh_stale(&store, &StreamFocus::default(), &mut drawn);
        let new = land_mesh(&mut loader, relit, &mut drawn);
        gpu.flush(&mut loader, |_| {});
        assert_eq!(
            loader.sections.resident(relit).map(|held| held.slot),
            Some(new),
            "the new mesh is placed rather than left waiting for room nobody gives back"
        );
        assert_eq!(gpu.drawn_sections(), [relit]);
    }

    fn a_mesh_read_out_of_a_column_the_server_took_back_is_thrown_away() {
        let stone = || stone_column(1);

        let mut loader = loader();
        let mut drawn = Messages::<SectionDrawn>::default();
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
        adopt(&mut loader, &mut store, &mut drawn);

        let centre = [0, 0, 0];
        let slot = loader.take_slot().expect("a free slot");
        loader.sections.dequeue(centre);
        loader.sections.start_meshing(centre);

        store.remove(ColumnPos::new(0, 0));
        adopt(&mut loader, &mut store, &mut drawn);
        assert!(
            loader.discard_orphan(centre, slot, &store, &StreamFocus::default()),
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

        loader.sections.start_meshing(centre);
        store.insert(ColumnPos::new(0, 0), stone());
        store.remove(ColumnPos::new(0, 0));
        store.insert(ColumnPos::new(0, 0), stone());
        adopt(&mut loader, &mut store, &mut drawn);
        assert!(
            loader.sections.work(centre) != Work::Queued,
            "the arrival cannot queue a section a task still holds"
        );
        assert!(loader.discard_orphan(centre, slot, &store, &StreamFocus::default()));
        assert!(
            loader.sections.work(centre) == Work::Queued,
            "the column is back, so the section is meshed again out of what it holds now"
        );
    }

    fn a_column_taken_back_before_it_is_adopted_leaves_the_section_count_where_it_was() {
        use mcrs_minecraft_client_world::columns::{Column, Extent, Section};

        let mut loader = loader();
        let mut drawn = Messages::<SectionDrawn>::default();
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
        adopt(&mut loader, &mut store, &mut drawn);

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
        app.add_plugins(StreamPlugin::new(
            Arc::new(budget()),
            Uploads::default(),
            MeshPacing {
                per_frame: 1,
                in_flight: 1,
            },
        ))
        .init_resource::<ColumnStore>()
        .insert_resource(Blocks(Arc::clone(
            &mcrs_minecraft_engine_registries::test_corpus().0.0,
        )))
        .add_message::<ColumnChange>()
        .add_message::<mcrs_minecraft_network::client::ConnectionEnded>();
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
        app.world_mut()
            .spawn((Camera3d::default(), GlobalTransform::default()));
        app.update();

        assert_eq!(app.world().resource::<Loader>().columns, 1);
    }

    #[test]
    fn a_section_the_loader_lays_and_evicts_reaches_the_cave() {
        let mut app = App::new();
        app.add_plugins(StreamPlugin::new(
            Arc::new(budget()),
            Uploads::default(),
            MeshPacing {
                per_frame: 1,
                in_flight: 1,
            },
        ))
        .insert_resource(CaveCull::new(1 << 8, true))
        .add_message::<mcrs_minecraft_network::client::ConnectionEnded>();
        let at = [1, 0, 1];
        let draw = |app: &mut App, step: fn(&mut Loader, &mut Messages<SectionDrawn>)| {
            app.world_mut()
                .resource_scope(|world, mut loader: Mut<Loader>| {
                    step(
                        &mut loader,
                        &mut world.resource_mut::<Messages<SectionDrawn>>(),
                    );
                });
            app.update();
            app.world_mut().resource_mut::<CaveCull>().walked_slot(at)
        };

        let laid = draw(&mut app, |loader, drawn| {
            loader
                .place(one_greedy_group([1, 0, 1], 5, 1), 5, drawn)
                .unwrap_or_else(|_| panic!("the arena has room"));
        });
        assert_eq!(laid, Some(5), "the cave lays the section the loader placed");

        let forgotten = draw(&mut app, |loader, drawn| loader.evict([1, 0, 1], drawn));
        assert_eq!(
            forgotten,
            Some(NO_SLOT),
            "and forgets it when the loader evicts it"
        );
    }

    fn stream_app(budget: Budget, pacing: MeshPacing) -> App {
        let mut app = App::new();
        app.add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            StreamPlugin::new(Arc::new(budget), Uploads::default(), pacing),
        ))
        .init_resource::<ColumnStore>()
        .insert_resource(Blocks(Arc::clone(
            &mcrs_minecraft_engine_registries::test_corpus().0.0,
        )))
        .insert_resource(CaveCull::new(1 << 8, true))
        .add_message::<ColumnChange>()
        .add_message::<mcrs_minecraft_network::client::ConnectionEnded>();
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(Vec3::splat(8.0)),
        ));
        app.update();
        app
    }

    fn status(app: &mut App) -> Status {
        use bevy::ecs::system::RunSystemOnce;
        app.world_mut()
            .run_system_once(|streaming: Streaming| streaming.status())
            .expect("the stream resources exist")
    }

    fn stone_column(sections: usize) -> mcrs_minecraft_client_world::columns::Column {
        use mcrs_minecraft_client_world::columns::{Column, Section};

        let stone = mcrs_voxel::BlockStateId::checked(1, 2).expect("a state past air");
        let section = || {
            Some(Section {
                blocks: Box::new([stone; mcrs_minecraft_mesh::SECTION_VOLUME]),
                biomes: PalettedContainer::Homogeneous(0),
                states: vec![stone],
            })
        };
        Column::unlit(0, (0..sections).map(|_| section()).collect())
    }

    fn insert_columns(
        app: &mut App,
        columns: impl IntoIterator<Item = ColumnPos>,
        sections: usize,
    ) {
        let mut changes = Vec::new();
        let mut store = app.world_mut().resource_mut::<ColumnStore>();
        if store.extent().is_none() {
            store.enter(Extent {
                min_section_y: 0,
                sections,
            });
        }
        for pos in columns {
            store.insert(pos, stone_column(sections));
        }
        store.drain_changes(&mut changes);
        app.world_mut().write_message_batch(changes);
        app.update();
    }

    fn around_origin() -> impl Iterator<Item = ColumnPos> {
        (-1..=1).flat_map(|x| (-1..=1).map(move |z| ColumnPos::new(x, z)))
    }

    fn catch_up_catalog(app: &mut App) {
        let mut catalog = app.world_mut().resource_mut::<BlockCatalog>();
        catalog.blocks = Arc::new(vec![BlockInfo::default()]);
        catalog.to_bake.clear();
    }

    fn start_mesh(app: &mut App, at: [i32; 3]) -> u32 {
        let mut loader = app.world_mut().resource_mut::<Loader>();
        let slot = loader.take_slot().expect("a free slot");
        loader.sections.start_meshing(at);
        let mesh = one_greedy_group(at, slot, 3);
        let task = AsyncComputeTaskPool::get().spawn(async move { (mesh, Scratch::default()) });
        loader.meshing.push((at, slot, task));
        slot
    }

    fn finish_meshes(app: &App) {
        let loader = app.world().resource::<Loader>();
        while !loader.meshing.iter().all(|(_, _, task)| task.is_finished()) {
            std::thread::yield_now();
        }
    }

    fn resident(app: &App, at: [i32; 3]) -> bool {
        app.world()
            .resource::<Loader>()
            .sections
            .resident(at)
            .is_some()
    }

    fn waits_for_room(app: &App, at: [i32; 3]) -> bool {
        app.world().resource::<Loader>().sections.work(at) == Work::Deferred
    }

    fn room_for_one_mesh() -> Budget {
        Budget {
            quads: 4,
            ..budget()
        }
    }

    const ONE_AT_A_TIME: MeshPacing = MeshPacing {
        per_frame: 1,
        in_flight: 1,
    };

    const FAR: [i32; 3] = [40, 0, 0];
    const NEAR: [i32; 3] = [0, 0, 0];

    fn land_far_then_near(app: &mut App) {
        for at in [FAR, NEAR] {
            start_mesh(app, at);
            finish_meshes(app);
            app.update();
        }
    }

    #[test]
    fn a_mesh_with_no_room_takes_the_place_of_the_farthest_resident_in_the_frame_it_lands() {
        let mut app = stream_app(room_for_one_mesh(), ONE_AT_A_TIME);
        land_far_then_near(&mut app);

        assert!(
            resident(&app, NEAR),
            "the near mesh is placed in the frame it lands"
        );
        assert!(!resident(&app, FAR), "in the room of the far one");
        assert_eq!(status(&mut app).evicted, 1);
        assert!(
            waits_for_room(&app, FAR),
            "the far section waits for room to come back"
        );
    }

    #[test]
    fn a_section_that_waited_for_room_is_queued_again_when_the_anchor_moves() {
        let mut app = stream_app(room_for_one_mesh(), ONE_AT_A_TIME);
        land_far_then_near(&mut app);
        let move_camera = |app: &mut App, x: f32| {
            let mut camera = app
                .world_mut()
                .query_filtered::<&mut GlobalTransform, With<Camera3d>>();
            *camera.single_mut(app.world_mut()).expect("one camera") =
                GlobalTransform::from_translation(Vec3::new(x, 8.0, 8.0));
            app.update();
            status(app).queued
        };

        assert_eq!(
            move_camera(&mut app, 8.0 + HYSTERESIS - 1.0),
            0,
            "a camera inside the hysteresis keeps the anchor"
        );
        assert_eq!(
            move_camera(&mut app, 8.0 + HYSTERESIS + 1.0),
            1,
            "a camera past it moves the anchor and queues the section that waited"
        );
    }

    #[test]
    fn a_section_laid_and_evicted_in_one_frame_reaches_the_cave_in_that_order() {
        let mut app = stream_app(room_for_one_mesh(), ONE_AT_A_TIME);
        let far = start_mesh(&mut app, FAR);
        let near = start_mesh(&mut app, NEAR);
        finish_meshes(&app);
        app.update();

        let told: Vec<SectionDrawn> = {
            let messages = app.world().resource::<Messages<SectionDrawn>>();
            messages.get_cursor().read(messages).copied().collect()
        };
        assert_eq!(
            told,
            [
                SectionDrawn::Laid {
                    section: FAR,
                    slot: far,
                    connectivity: mcrs_minecraft_mesh::OPEN,
                },
                SectionDrawn::Forgotten { section: FAR },
                SectionDrawn::Laid {
                    section: NEAR,
                    slot: near,
                    connectivity: mcrs_minecraft_mesh::OPEN,
                },
            ]
        );
        let mut cave = app.world_mut().resource_mut::<CaveCull>();
        assert_eq!(
            [cave.walked_slot(FAR), cave.walked_slot(NEAR)],
            [Some(NO_SLOT), Some(near)],
            "the cave forgets the section the stream laid and evicted, and draws the one in its place"
        );
    }

    #[test]
    fn a_column_sent_again_ends_with_its_sections_queued() {
        let mut app = stream_app(budget(), ONE_AT_A_TIME);
        insert_columns(&mut app, around_origin(), 1);
        assert_eq!(status(&mut app).queued, 1, "only the centre is surrounded");
        start_mesh(&mut app, NEAR);
        finish_meshes(&app);
        app.update();
        assert!(
            resident(&app, NEAR),
            "the centre is drawn before it is sent again"
        );

        insert_columns(&mut app, [ColumnPos::new(0, 0)], 1);
        let after = status(&mut app);
        assert!(
            !resident(&app, NEAR),
            "the mesh of the column it replaced is gone"
        );
        assert_eq!(
            after.queued, 1,
            "and the section of the column sent again is meshed again"
        );
        assert_eq!(after.columns, 9);
    }

    #[test]
    fn a_section_held_back_by_the_slots_or_the_pacing_is_admitted_in_the_frame_room_frees() {
        let one_slot = Budget {
            sections: 1,
            ..budget()
        };
        let plenty = MeshPacing {
            per_frame: 4,
            in_flight: 4,
        };
        for (case, budget, pacing) in [
            ("one slot", one_slot, plenty),
            ("one in flight", budget(), ONE_AT_A_TIME),
        ] {
            let mut app = stream_app(budget, pacing);
            insert_columns(&mut app, around_origin(), 2);
            catch_up_catalog(&mut app);
            app.update();
            let first = status(&mut app);
            assert_eq!(
                [first.meshing, first.queued],
                [1, 1],
                "{case}: one section is admitted and the other waits"
            );

            finish_meshes(&app);
            app.update();
            let second = status(&mut app);
            assert_eq!(
                [second.meshing, second.queued, second.sections],
                [1, 0, 1],
                "{case}: the frame that collects the first mesh admits the second"
            );
        }
    }

    mod exhaustive {
        use super::*;

        #[test]
        fn a_bucket_the_arena_cannot_rebuild_beside_its_block_stops_sending_itself_again() {
            let mut loader = Loader::new(&Budget {
                quads: 1 << 13,
                models: 1 << 12,
                faces: 1 << 16,
                groups: 1 << 12,
                sections: 1 << 13,
                upload: 0,
                tint_size: [512; 2],
            });
            let mut drawn = Messages::<SectionDrawn>::default();

            let mut last = Vec::new();
            for index in 0..3000u32 {
                loader
                    .place(
                        one_greedy_group([index as i32, 0, 0], index, 1),
                        index,
                        &mut drawn,
                    )
                    .unwrap_or_else(|_| panic!("the arena has room"));
                last = flushed(&mut loader, &StreamFocus::default());
            }

            let draws = last
                .iter()
                .find_map(|placement| placement.draws.as_ref())
                .expect("a flush hands over the whole draw list");
            assert_eq!(
                draws[0].group_count,
                1 << 11,
                "a bucket past what the arena can give it draws what its block holds"
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
