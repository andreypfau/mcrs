use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_resource::binding_types::{sampler, texture_3d, uniform_buffer_sized};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::layout::{
    JOB_WORDS, Lane, REGION_CELLS, SLOT_ABOVE, SLOT_UNLOADED, job_words, lane_words, neighbours,
};
use mcrs_minecraft_render::sky::ExtractedSky;
use mcrs_minecraft_render::{Brightness, RenderPath, uniform_buffer};

pub(crate) const BRICK_SIDE: u32 = 18;
const BRICK_BYTES: u64 = (SectionPos::VOLUME * 4) as u64;
const LANE_WORDS_PER_JOB: u32 = 4;
const MAX_SLOTS: u32 = 1 << 16;
const PENDING: u32 = 1 << 31;

// chisle: 9.5 atlas slots per column rests on the densest Nether window measured, 7.7 lit and
// 9.1 propagated sections per column; a denser region reads the vanilla tint.
const ATLAS_HALF_SLOTS_PER_COLUMN: u32 = 19;
// chisle: 12 bricks per column rests on the 11.5 measured around Nether emitters; a denser
// region reads the vanilla tint.
const POOL_BRICKS_PER_COLUMN: u64 = 12;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, ExtractResource)]
pub struct VolumeSettings {
    pub radius: u8,
    pub view_distance: u8,
    pub sections_per_frame: u32,
}

impl Default for VolumeSettings {
    fn default() -> Self {
        Self {
            radius: 0,
            view_distance: 2,
            sections_per_frame: 1,
        }
    }
}

pub enum VolumeCommand {
    Reset { min_section_y: i32, sections: u32 },
    Brick { section: IVec3, words: Box<[u32]> },
    Evict { section: IVec3 },
    Dirty { section: IVec3, lanes: Vec<Lane> },
}

/// Filled by the client, drained by the render world. A new generation means the render world
/// dropped every command and every slot, so whoever fills it starts again.
#[derive(Resource, Clone, Default)]
pub struct VolumeQueue(Arc<Mutex<Inbox>>);

#[derive(Default)]
struct Inbox {
    generation: u64,
    radius: u8,
    pending: usize,
    commands: VecDeque<VolumeCommand>,
}

impl VolumeQueue {
    pub fn push(&self, command: VolumeCommand) {
        self.inbox().commands.push_back(command);
    }

    pub fn generation(&self) -> u64 {
        self.inbox().generation
    }

    /// The radius the render world can hold, which may be less than the one asked for.
    pub fn radius(&self) -> u8 {
        self.inbox().radius
    }

    pub fn idle(&self) -> bool {
        let inbox = self.inbox();
        inbox.commands.is_empty() && inbox.pending == 0
    }

    fn inbox(&self) -> MutexGuard<'_, Inbox> {
        self.0.lock().unwrap()
    }

    fn restart(&self, radius: u8) {
        let mut inbox = self.inbox();
        inbox.generation += 1;
        inbox.radius = radius;
        inbox.pending = 0;
        inbox.commands.clear();
    }

    fn close(&self) {
        let mut inbox = self.inbox();
        inbox.radius = 0;
        inbox.pending = 0;
        inbox.commands.clear();
    }

    fn take(&self, into: &mut Vec<VolumeCommand>) {
        into.extend(self.inbox().commands.drain(..));
    }

    fn set_pending(&self, pending: usize) {
        self.inbox().pending = pending;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub(crate) enum Tier {
    None = 0,
    Full = 1,
}

/// One `Rg32Uint` texel: the atlas slot in bits 0-15 of R, the tier in 16-19, pending in 31.
/// G is reserved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageEntry {
    pub slot: u32,
    pub tier: Tier,
    pub pending: bool,
}

impl PageEntry {
    fn texel(self) -> [u32; 2] {
        let pending = if self.pending { PENDING } else { 0 };
        [self.slot | (self.tier as u32) << 16 | pending, 0]
    }
}

/// Toroidal in x and z over a power-of-two width no narrower than the render distance, so no two
/// resident columns share a page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Pages {
    width: u32,
    min_section_y: i32,
    rows: u32,
}

impl Pages {
    fn new(view_distance: u8, min_section_y: i32, rows: u32) -> Self {
        Self {
            width: (2 * view_distance as u32 + 1).next_power_of_two(),
            min_section_y,
            rows,
        }
    }

    fn index(&self, section: IVec3) -> Option<UVec3> {
        let row = section.y.checked_sub(self.min_section_y)?;
        let wrap = self.width as i32 - 1;
        (0..self.rows as i32).contains(&row).then(|| {
            UVec3::new(
                (section.x & wrap) as u32,
                row as u32,
                (section.z & wrap) as u32,
            )
        })
    }

    fn extent(&self) -> UVec3 {
        UVec3::new(self.width, self.rows.max(1), self.width)
    }

    fn above(&self, section: IVec3) -> bool {
        section.y >= self.min_section_y + self.rows as i32
    }
}

/// Slots of 18³ texels packed `per_axis` to a side, x fastest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct AtlasGrid {
    per_axis: UVec3,
}

impl AtlasGrid {
    fn holding(slots: u32, max_dimension: u32) -> Self {
        let most = (max_dimension / BRICK_SIDE).max(1);
        let a = (1..=most).find(|a| a * a * a >= slots).unwrap_or(most);
        let b = (1..=most).find(|b| a * b * b >= slots).unwrap_or(most);
        let c = slots.div_ceil(a * b).clamp(1, most);
        Self {
            per_axis: UVec3::new(a, b, c),
        }
    }

    fn capacity(&self) -> u32 {
        self.per_axis.element_product().min(MAX_SLOTS)
    }

    fn origin(&self, slot: u32) -> UVec3 {
        let UVec3 { x: a, y: b, .. } = self.per_axis;
        BRICK_SIDE * UVec3::new(slot % a, slot / a % b, slot / (a * b))
    }

    fn extent(&self) -> UVec3 {
        self.per_axis * BRICK_SIDE
    }
}

fn atlas_slots(radius: u8) -> u32 {
    if radius == 0 {
        return 1;
    }
    let side = 2 * radius as u32 + 1;
    side * side * ATLAS_HALF_SLOTS_PER_COLUMN / 2 + 1
}

fn pool_bricks(radius: u8) -> u64 {
    let side = 2 * radius as u64 + 3;
    side * side * POOL_BRICKS_PER_COLUMN
}

fn fit_radius(requested: u8, pool_bytes: u64) -> u8 {
    (0..=requested)
        .rev()
        .find(|&radius| pool_bricks(radius) * BRICK_BYTES <= pool_bytes)
        .unwrap_or(0)
}

fn chebyshev(a: IVec3, b: IVec3) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

#[derive(Default)]
pub(crate) struct Jobs {
    pub records: Vec<[u32; JOB_WORDS]>,
    pub lane_words: u32,
    pub widest: u32,
}

/// The render world's index of what the pool, the atlas and the page table hold.
pub(crate) struct VolumeBook {
    grid: AtlasGrid,
    view_distance: u8,
    pages: Option<Pages>,
    pool_capacity: u32,
    pool: HashMap<IVec3, u32>,
    free_pool: Vec<u32>,
    pool_next: u32,
    atlas: HashMap<IVec3, u32>,
    free_atlas: Vec<u32>,
    atlas_next: u32,
    pending: HashMap<IVec3, Vec<Lane>>,
    touched: HashSet<IVec3>,
}

impl VolumeBook {
    fn new(pool_capacity: u32, grid: AtlasGrid, view_distance: u8) -> Self {
        Self {
            grid,
            view_distance,
            pages: None,
            pool_capacity,
            pool: HashMap::default(),
            free_pool: Vec::new(),
            pool_next: 0,
            atlas: HashMap::default(),
            free_atlas: Vec::new(),
            atlas_next: 1,
            pending: HashMap::default(),
            touched: HashSet::default(),
        }
    }

    fn reset(&mut self, min_section_y: i32, sections: u32) {
        *self = Self::new(self.pool_capacity, self.grid, self.view_distance);
        self.pages = Some(Pages::new(self.view_distance, min_section_y, sections));
    }

    fn brick(&mut self, section: IVec3) -> Option<u32> {
        if let Some(&slot) = self.pool.get(&section) {
            return Some(slot);
        }
        let slot = self.free_pool.pop().or_else(|| {
            (self.pool_next < self.pool_capacity).then(|| {
                self.pool_next += 1;
                self.pool_next - 1
            })
        })?;
        self.pool.insert(section, slot);
        Some(slot)
    }

    fn dirty(&mut self, section: IVec3, lanes: Vec<Lane>) {
        if self.pending.insert(section, lanes).is_none() {
            self.touched.insert(section);
        }
    }

    fn evict(&mut self, section: IVec3) {
        if let Some(slot) = self.pool.remove(&section) {
            self.free_pool.push(slot);
        }
        let coloured = self
            .atlas
            .remove(&section)
            .map(|slot| self.free_atlas.push(slot));
        if self.pending.remove(&section).is_some() || coloured.is_some() {
            self.touched.insert(section);
        }
    }

    fn entry(&self, section: IVec3) -> PageEntry {
        let slot = self.atlas.get(&section).copied();
        PageEntry {
            slot: slot.unwrap_or(0),
            tier: if slot.is_some() {
                Tier::Full
            } else {
                Tier::None
            },
            pending: self.pending.contains_key(&section),
        }
    }

    fn atlas_slot(&mut self, section: IVec3) -> Option<u32> {
        if let Some(&slot) = self.atlas.get(&section) {
            return Some(slot);
        }
        let capacity = self.grid.capacity();
        let slot = self.free_atlas.pop().or_else(|| {
            (self.atlas_next < capacity).then(|| {
                self.atlas_next += 1;
                self.atlas_next - 1
            })
        })?;
        self.atlas.insert(section, slot);
        Some(slot)
    }

    fn neighbour_slot(&self, section: IVec3, pages: Pages) -> u32 {
        if pages.above(section) {
            SLOT_ABOVE
        } else {
            self.pool.get(&section).copied().unwrap_or(SLOT_UNLOADED)
        }
    }

    /// The nearest pending sections within `radius` columns, up to `n` that some light reaches.
    /// Jobs are taken while their lane words fit `lane_budget`; the first is taken whatever its
    /// width. A section no light reaches is published neutral without a job.
    fn take_jobs(&mut self, camera: IVec3, radius: u8, n: usize, lane_budget: u32) -> Jobs {
        let mut jobs = Jobs::default();
        let (Some(pages), true) = (self.pages, radius > 0) else {
            return jobs;
        };
        let mut near: Vec<IVec3> = self
            .pending
            .keys()
            .copied()
            .filter(|&section| {
                chebyshev(camera, section) <= radius as i32 && pages.index(section).is_some()
            })
            .collect();
        near.sort_unstable_by_key(|&section| (section - camera).length_squared());
        for section in near {
            let lanes = &self.pending[&section];
            if lanes.is_empty() {
                self.pending.remove(&section);
                if let Some(slot) = self.atlas.remove(&section) {
                    self.free_atlas.push(slot);
                }
                self.touched.insert(section);
                continue;
            }
            let words = lane_words(lanes.len());
            let full = jobs.records.len() == n
                || (!jobs.records.is_empty() && jobs.lane_words + words > lane_budget);
            if full {
                continue;
            }
            let Some(slot) = self.atlas_slot(section) else {
                continue;
            };
            let mut slots = [SLOT_UNLOADED; 27];
            for (entry, neighbour) in slots.iter_mut().zip(neighbours(SectionPos(section))) {
                *entry = self.neighbour_slot(neighbour.0, pages);
            }
            let lanes = self
                .pending
                .remove(&section)
                .expect("taken from the pending keys");
            let lane_base = jobs.lane_words * REGION_CELLS as u32;
            let origin = self.grid.origin(slot).to_array();
            jobs.records
                .push(job_words(&lanes, slots, lane_base, origin));
            jobs.lane_words += words;
            jobs.widest = jobs.widest.max(words);
            self.touched.insert(section);
        }
        jobs
    }

    fn take_touched(&mut self) -> Vec<(UVec3, [u32; 2])> {
        let touched = std::mem::take(&mut self.touched);
        let Some(pages) = self.pages else {
            return Vec::new();
        };
        touched
            .into_iter()
            .filter_map(|section| Some((pages.index(section)?, self.entry(section).texel())))
            .collect()
    }

    fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Copy, Clone, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct VolumeTerms {
    ambient: [f32; 4],
    sky_light: [f32; 4],
    block_light: [f32; 4],
    brightness: f32,
    radius: i32,
    min_section_y: i32,
    _pad: i32,
}

const TERMS_SIZE: u64 = size_of::<VolumeTerms>() as u64;

pub(crate) fn volume_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "light volume",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_3d(TextureSampleType::Uint),
                texture_3d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer_sized(false, NonZeroU64::new(TERMS_SIZE)),
            ),
        ),
    )
}

pub(crate) struct Scratch {
    pub jobs: Buffer,
    pub cost: Buffer,
    pub lanes: [Buffer; 2],
    pub lane_words: u32,
}

fn storage(label: &str, bytes: u64, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: bytes.max(4),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn lanes(words: u32, device: &RenderDevice) -> [Buffer; 2] {
    let bytes = words as u64 * REGION_CELLS as u64 * 4;
    [(); 2].map(|()| storage("light colour lanes", bytes, device))
}

/// Everything the light volume allocates. Nothing else may hold a clone of these buffers,
/// textures or bind groups, or switching back to classic would not free them.
#[derive(Resource)]
pub(crate) struct Volume {
    pub radius: u8,
    pub jobs_per_frame: u32,
    pub book: VolumeBook,
    pub pool: Buffer,
    pub atlas: TextureView,
    pub scratch: Scratch,
    pub colour_groups: Option<[BindGroup; 2]>,
    pub bind_group: BindGroup,
    settings: VolumeSettings,
    page_table: Texture,
    sampler: Sampler,
    terms: Buffer,
    terms_written: Option<VolumeTerms>,
}

impl Volume {
    fn new(
        settings: VolumeSettings,
        device: &RenderDevice,
        queue: &RenderQueue,
        cache: &PipelineCache,
    ) -> Self {
        let limits = device.limits();
        let binding = (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size);
        let radius = fit_radius(settings.radius, binding);
        let pool_slots = if radius == 0 {
            1
        } else {
            (binding / BRICK_BYTES).min(pool_bricks(radius)) as u32
        };
        let per_job = LANE_WORDS_PER_JOB as u64 * REGION_CELLS as u64 * 4;
        let jobs_per_frame = settings
            .sections_per_frame
            .clamp(1, (binding / per_job).max(1) as u32);
        let grid = AtlasGrid::holding(atlas_slots(radius), limits.max_texture_dimension_3d);

        let pool = storage(
            "light volume bricks",
            pool_slots as u64 * BRICK_BYTES,
            device,
        );
        let atlas_texture = texture_3d_of(
            "light volume atlas",
            grid.extent(),
            TextureFormat::Rgba8Unorm,
            TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST,
            device,
        );
        let neutral = [[0u8, 0, 0, 255]; (BRICK_SIDE * BRICK_SIDE * BRICK_SIDE) as usize];
        write_texels(
            queue,
            &atlas_texture,
            UVec3::ZERO,
            UVec3::splat(BRICK_SIDE),
            4,
            bytemuck::cast_slice(&neutral),
        );
        let atlas = atlas_texture.create_view(&TextureViewDescriptor::default());
        let page_table = page_table(UVec3::ONE, device);
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("light volume"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });
        let terms = uniform_buffer("light volume terms", TERMS_SIZE, device);
        let bind_group = volume_bind_group(&page_table, &atlas, &sampler, &terms, device, cache);
        let scratch = Scratch {
            jobs: storage(
                "light colour jobs",
                jobs_per_frame as u64 * JOB_WORDS as u64 * 4,
                device,
            ),
            cost: storage(
                "light colour cost",
                jobs_per_frame as u64 * REGION_CELLS as u64 * 4,
                device,
            ),
            lanes: lanes(jobs_per_frame * LANE_WORDS_PER_JOB, device),
            lane_words: jobs_per_frame * LANE_WORDS_PER_JOB,
        };
        info!(
            radius,
            requested = settings.radius,
            bricks = pool_slots,
            atlas_slots = grid.capacity() - 1,
            atlas = ?grid.extent(),
            sections_per_frame = jobs_per_frame,
            "the light volume is ready"
        );
        Self {
            radius,
            jobs_per_frame,
            book: VolumeBook::new(pool_slots, grid, settings.view_distance),
            pool,
            atlas,
            scratch,
            colour_groups: None,
            bind_group,
            settings,
            page_table,
            sampler,
            terms,
            terms_written: None,
        }
    }

    fn reset(
        &mut self,
        min_section_y: i32,
        sections: u32,
        device: &RenderDevice,
        cache: &PipelineCache,
    ) {
        self.book.reset(min_section_y, sections);
        let pages = self.book.pages.expect("a reset book has pages");
        self.page_table = page_table(pages.extent(), device);
        self.bind_group = volume_bind_group(
            &self.page_table,
            &self.atlas,
            &self.sampler,
            &self.terms,
            device,
            cache,
        );
        self.terms_written = None;
    }

    pub fn grow_lanes(&mut self, words: u32, device: &RenderDevice) {
        warn_once!(
            words,
            "a light colour job needs more lane words than a frame's scratch holds; it grows"
        );
        self.scratch.lanes = lanes(words, device);
        self.scratch.lane_words = words;
        self.colour_groups = None;
    }

    pub fn take_jobs(&mut self, camera: IVec3) -> Jobs {
        self.book.take_jobs(
            camera,
            self.radius,
            self.jobs_per_frame as usize,
            self.scratch.lane_words,
        )
    }

    pub fn publish(&mut self, queue: &RenderQueue) {
        for (page, texel) in self.book.take_touched() {
            write_texels(
                queue,
                &self.page_table,
                page,
                UVec3::ONE,
                8,
                bytemuck::cast_slice(&texel),
            );
        }
    }
}

fn texture_3d_of(
    label: &str,
    size: UVec3,
    format: TextureFormat,
    usage: TextureUsages,
    device: &RenderDevice,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: size.z,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D3,
        format,
        usage,
        view_formats: &[],
    })
}

fn page_table(size: UVec3, device: &RenderDevice) -> Texture {
    texture_3d_of(
        "light volume pages",
        size,
        TextureFormat::Rg32Uint,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        device,
    )
}

fn write_texels(
    queue: &RenderQueue,
    texture: &Texture,
    origin: UVec3,
    size: UVec3,
    texel_bytes: u32,
    data: &[u8],
) {
    queue.write_texture(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Origin3d {
                x: origin.x,
                y: origin.y,
                z: origin.z,
            },
            aspect: TextureAspect::All,
        },
        data,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.x * texel_bytes),
            rows_per_image: Some(size.y),
        },
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: size.z,
        },
    );
}

fn volume_bind_group(
    page_table: &Texture,
    atlas: &TextureView,
    sampler: &Sampler,
    terms: &Buffer,
    device: &RenderDevice,
    cache: &PipelineCache,
) -> BindGroup {
    let pages = page_table.create_view(&TextureViewDescriptor::default());
    device.create_bind_group(
        "light volume",
        &cache.get_bind_group_layout(&volume_layout()),
        &BindGroupEntries::sequential((&pages, atlas, sampler, terms.as_entire_binding())),
    )
}

pub(crate) fn fit_volume(
    mut commands: Commands,
    requested: Res<RenderPath>,
    settings: Res<VolumeSettings>,
    volume: Option<Res<Volume>>,
    queue: Res<VolumeQueue>,
    device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
) {
    if *requested == RenderPath::Classic {
        if volume.is_some() {
            commands.remove_resource::<Volume>();
        }
        queue.close();
        return;
    }
    if volume.is_some_and(|volume| volume.settings == *settings) {
        return;
    }
    let volume = Volume::new(*settings, &device, &render_queue, &cache);
    queue.restart(volume.radius);
    commands.insert_resource(volume);
}

pub(crate) fn apply_volume_commands(
    volume: Option<ResMut<Volume>>,
    queue: Res<VolumeQueue>,
    device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
    mut inbox: Local<Vec<VolumeCommand>>,
) {
    let Some(mut volume) = volume else {
        return;
    };
    let volume = &mut *volume;
    queue.take(&mut inbox);
    for command in inbox.drain(..) {
        match command {
            VolumeCommand::Reset {
                min_section_y,
                sections,
            } => volume.reset(min_section_y, sections, &device, &cache),
            VolumeCommand::Brick { section, words } => {
                debug_assert_eq!(words.len() as u64 * 4, BRICK_BYTES);
                if let Some(slot) = volume.book.brick(section) {
                    render_queue.write_buffer(
                        &volume.pool,
                        slot as u64 * BRICK_BYTES,
                        bytemuck::cast_slice(&words),
                    );
                }
            }
            VolumeCommand::Evict { section } => volume.book.evict(section),
            VolumeCommand::Dirty { section, lanes } => volume.book.dirty(section, lanes),
        }
    }
    volume.publish(&render_queue);
    queue.set_pending(volume.book.pending_len());
}

/// The terms follow the light colours, the brightness and the extent, which move at tick rate
/// and often not at all, so a frame whose inputs match the last one uploads nothing.
pub(crate) fn write_volume_terms(
    volume: Option<ResMut<Volume>>,
    sky: Option<Res<ExtractedSky>>,
    brightness: Res<Brightness>,
    queue: Res<RenderQueue>,
) {
    let (Some(mut volume), Some(sky)) = (volume, sky) else {
        return;
    };
    let terms = VolumeTerms {
        ambient: sky.uniform.ambient,
        sky_light: sky.uniform.sky_light,
        block_light: sky.uniform.block_light,
        brightness: brightness.0,
        radius: volume.radius as i32,
        min_section_y: volume.book.pages.map_or(0, |pages| pages.min_section_y),
        _pad: 0,
    };
    if volume.terms_written == Some(terms) {
        return;
    }
    queue.write_buffer(&volume.terms, 0, bytemuck::bytes_of(&terms));
    volume.terms_written = Some(terms);
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_light_color::colors::LightType;

    use super::*;

    /// What the shader reads out of a page texel.
    fn read(texel: [u32; 2]) -> PageEntry {
        let bits = texel[0];
        PageEntry {
            slot: bits & 0xffff,
            tier: if (bits >> 16) & 0xf == Tier::Full as u32 {
                Tier::Full
            } else {
                Tier::None
            },
            pending: bits & PENDING != 0,
        }
    }

    fn lanes_of(types: u8) -> Vec<Lane> {
        (1..=types)
            .map(|t| Lane {
                light_type: LightType(t),
                colour: Some([t, 0, 0]),
            })
            .collect()
    }

    fn book(radius: u8) -> VolumeBook {
        VolumeBook::new(64, AtlasGrid::holding(atlas_slots(radius), 2048), 8)
    }

    #[test]
    fn page_indices_wrap_negative_sections_and_hold_at_the_world_edge() {
        let view = 96;
        let pages = Pages::new(view, -4, 24);
        assert_eq!(pages.width, 256);
        let edge = 30_000_000 / 16;
        for x in [-1, 0, edge - 1, edge, -edge] {
            let at = pages
                .index(IVec3::new(x, 0, -x))
                .expect("a row of the dimension");
            assert!(at.x < pages.width && at.z < pages.width, "{x}: {at}");
        }
        for camera in [IVec3::ZERO, IVec3::new(edge - 2, 5, -edge + 3)] {
            let mut seen = HashSet::new();
            let reach = view as i32;
            for dz in -reach..=reach {
                for dx in -reach..=reach {
                    let at = pages.index(camera + IVec3::new(dx, 0, dz)).unwrap();
                    assert!(seen.insert(at), "{camera} + ({dx}, {dz}) shares {at}");
                }
            }
        }
        let width = pages.width as i32;
        assert_eq!(
            pages.index(IVec3::new(edge, 3, -1)),
            pages.index(IVec3::new(edge - width, 3, -1 + width))
        );
        assert_eq!(pages.index(IVec3::new(0, -5, 0)), None);
        assert_eq!(pages.index(IVec3::new(0, 20, 0)), None);
        assert_eq!(pages.index(IVec3::new(0, 19, 0)).map(|at| at.y), Some(23));
    }

    #[test]
    fn a_page_entry_holds_its_slot_tier_and_pending_bit() {
        for slot in [0, 1, 4351, 0xffff] {
            for tier in [Tier::None, Tier::Full] {
                for pending in [false, true] {
                    let entry = PageEntry {
                        slot,
                        tier,
                        pending,
                    };
                    let texel = entry.texel();
                    assert_eq!(texel[1], 0, "{entry:?}");
                    assert_eq!(read(texel), entry);
                }
            }
        }
        let neutral = PageEntry {
            slot: 0,
            tier: Tier::None,
            pending: false,
        };
        assert_eq!(neutral.texel(), [0, 0], "a zeroed page table is neutral");

        let mut book = book(2);
        book.reset(0, 4);
        let section = IVec3::new(0, 1, 0);
        book.dirty(section, lanes_of(1));
        assert_eq!(
            book.entry(section),
            PageEntry {
                slot: 0,
                tier: Tier::None,
                pending: true
            }
        );
    }

    #[test]
    fn atlas_slots_tile_inside_the_3d_limit() {
        for radius in [6, 10, 11] {
            let side = 2 * radius as u32 + 1;
            let wanted = atlas_slots(radius);
            assert!(wanted > side * side * 19 / 2, "{radius}");
            let grid = AtlasGrid::holding(wanted, 2048);
            assert!(grid.capacity() >= wanted, "{radius}: {grid:?}");
            let extent = grid.extent();
            assert!(extent.max_element() <= 2048, "{radius}: {extent}");
            let mut origins = HashSet::new();
            for slot in 0..grid.capacity() {
                let origin = grid.origin(slot);
                assert!(
                    (origin + BRICK_SIDE).cmple(extent).all(),
                    "{slot}: {origin}"
                );
                assert!(origins.insert(origin), "{slot} shares {origin}");
            }
        }
        let off = AtlasGrid::holding(atlas_slots(0), 2048);
        assert_eq!(off.capacity(), 1, "only the neutral slot");
        assert_eq!(off.extent(), UVec3::splat(BRICK_SIDE));
        let narrow = AtlasGrid::holding(atlas_slots(10), 256);
        assert!(narrow.extent().max_element() <= 256);
    }

    #[test]
    fn the_radius_is_clamped_to_what_the_pool_binding_holds() {
        let binding = 128 << 20;
        assert_eq!(fit_radius(10, binding), 10);
        assert_eq!(fit_radius(11, binding), 11);
        assert_eq!(fit_radius(12, binding), 11);
        assert_eq!(fit_radius(96, binding), 11);
        assert_eq!(fit_radius(0, binding), 0);
        assert_eq!(fit_radius(4, BRICK_BYTES), 0);
    }

    #[test]
    fn a_job_names_the_pool_slot_the_unloaded_and_the_above_sentinels() {
        let mut book = book(4);
        book.reset(0, 4);
        let top = IVec3::new(5, 3, -7);
        let built = [
            top,
            top + IVec3::X,
            top - IVec3::Y,
            top + IVec3::new(-1, -1, 1),
        ];
        let pool: Vec<u32> = built.iter().map(|&s| book.brick(s).unwrap()).collect();
        let lanes = lanes_of(2);
        book.dirty(top, lanes.clone());
        let jobs = book.take_jobs(top, 4, 8, 64);
        assert_eq!(jobs.records.len(), 1);

        let index = |d: IVec3| ((d.x + 1) + 3 * ((d.z + 1) + 3 * (d.y + 1))) as usize;
        let mut slots = [SLOT_UNLOADED; 27];
        slots[18..].fill(SLOT_ABOVE);
        slots[index(IVec3::ZERO)] = pool[0];
        slots[index(IVec3::X)] = pool[1];
        slots[index(-IVec3::Y)] = pool[2];
        slots[index(IVec3::new(-1, -1, 1))] = pool[3];
        assert_eq!(slots[index(-IVec3::X)], SLOT_UNLOADED);
        let origin = book.grid.origin(1).to_array();
        assert_eq!(jobs.records[0], job_words(&lanes, slots, 0, origin));
        assert_eq!(
            book.entry(top),
            PageEntry {
                slot: 1,
                tier: Tier::Full,
                pending: false
            }
        );
    }

    #[test]
    fn the_nearest_pending_sections_run_first() {
        let mut book = book(8);
        book.reset(0, 8);
        let camera = IVec3::new(100, 2, -40);
        for offset in [
            IVec3::new(3, 0, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(-5, 1, 2),
            IVec3::new(1, 1, 1),
            IVec3::new(0, 4, -6),
            IVec3::new(9, 0, 0),
        ] {
            book.dirty(camera + offset, lanes_of(1));
        }
        assert_eq!(book.take_jobs(camera, 8, 2, 64).records.len(), 2);
        let coloured: HashSet<IVec3> = book.atlas.keys().copied().collect();
        let nearest = [camera + IVec3::new(0, 0, 1), camera + IVec3::new(1, 1, 1)];
        assert_eq!(coloured, HashSet::from(nearest));
        assert_eq!(book.take_jobs(camera, 8, 8, 64).records.len(), 3);
        assert_eq!(book.pending_len(), 1, "the section past the radius waits");
    }

    #[test]
    fn a_frame_packs_jobs_until_their_lane_words_fill_the_scratch() {
        let mut book = book(4);
        book.reset(0, 4);
        for x in 0..3 {
            book.dirty(IVec3::new(x, 0, 0), lanes_of(5));
        }
        let jobs = book.take_jobs(IVec3::ZERO, 4, 8, 4);
        assert_eq!((jobs.records.len(), jobs.lane_words), (2, 4));
        let jobs = book.take_jobs(IVec3::ZERO, 4, 8, 1);
        assert_eq!(
            (jobs.records.len(), jobs.lane_words, jobs.widest),
            (1, 2, 2),
            "a job wider than the scratch runs alone"
        );
    }

    #[test]
    fn an_empty_palette_publishes_the_neutral_entry_without_a_job() {
        let mut book = book(4);
        book.reset(-4, 8);
        let section = IVec3::new(-1, -4, -1);
        book.dirty(section, lanes_of(1));
        assert_eq!(book.take_jobs(section, 4, 8, 64).records.len(), 1);
        book.take_touched();

        book.dirty(section, Vec::new());
        assert!(book.take_jobs(section, 4, 8, 64).records.is_empty());
        assert_eq!(
            book.entry(section),
            PageEntry {
                slot: 0,
                tier: Tier::None,
                pending: false
            }
        );
        let page = book.pages.unwrap().index(section).unwrap();
        assert_eq!(book.take_touched(), [(page, [0, 0])]);

        let other = section + IVec3::X;
        book.dirty(other, lanes_of(1));
        book.take_jobs(section, 4, 8, 64);
        assert_eq!(book.entry(other).slot, 1, "the freed slot is taken again");
    }
}
