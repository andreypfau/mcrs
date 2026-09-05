//! One batch of edits, turned into new light, split so the expensive part can
//! leave the tick loop: [`LightWorld::prepare`] runs on the caller's thread,
//! [`LightJob::run`] is pure computation holding no borrows, and
//! [`LightWorld::apply`] publishes the result.

use rayon::prelude::*;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mcrs_voxel_math::{BlockPos, ChunkPos, ColumnPos};
use mcrs_voxel_storage::VoxelId;

use crate::block::{Layer, LightRegistry};
use crate::field::{BlockSnapshot, CellIndex, FieldLayout, LightField, SectionSource, block_in};
use crate::level::{BlockColumn, LightLevel, LocalPos};
use crate::region::{BlockBox, ErasePlan, INFLUENCE_RADIUS, Influence, Regions};
use crate::relax::relax;
use crate::section;
use crate::sky::SkyFloors;
use crate::storage::LightStorage;
use crate::world::{Edit, LightWorld, Section, local_of};

/// New light for one section.
#[derive(Clone, Debug)]
pub struct SectionLight {
    pub pos: ChunkPos,
    pub block_light: LightStorage,
    pub sky_light: LightStorage,
}

/// The finished result of an epoch, ready to be published.
#[derive(Clone, Debug)]
pub struct LightUpdate {
    pub sections: Vec<SectionLight>,
    pub stats: EpochStats,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct EpochStats {
    pub block_rounds: usize,
    pub sky_rounds: usize,
    pub erased_cells: usize,
    pub area_cells: usize,
    pub timings: EpochTimings,
}

/// Below this many cells the seeding pass is run on one thread: a small field
/// costs less to walk than a thread pool costs to start.
const PARALLEL_SEED_LIMIT: usize = 1 << 18;

/// Where the time inside one epoch went. Costs a handful of clock reads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct EpochTimings {
    /// Allocating the working field and copying published light into it.
    pub fill: Duration,
    /// Erasing and collecting seeds.
    pub seed: Duration,
    /// The calculation itself.
    pub relax: Duration,
    /// Reading the result back out.
    pub read_back: Duration,
}

/// Returning the old value instead of the equal new one is not a no-op: it
/// keeps the buffer the world already points at, so the later publish settles
/// for a pointer compare.
fn keep_published(published: Option<LightStorage>, computed: LightStorage) -> LightStorage {
    match published {
        Some(old) if old == computed => old,
        _ => computed,
    }
}

/// A prepared, self-contained unit of work. Holds no borrows on the world, so
/// it can be sent to a worker thread and outlive further edits to the world.
pub struct LightJob {
    registry: Arc<LightRegistry>,
    layout: FieldLayout,
    blocks: BlockSnapshot,
    /// Published light for each section of the working area, cloned rather than
    /// expanded: building the field is the expensive part and belongs on the
    /// worker, not on the thread that asked for the work.
    published: Vec<Option<(LightStorage, LightStorage)>>,
    erase_plan: ErasePlan,
    sky_floors: SkyFloors,
}

impl LightJob {
    /// The sections this job will publish. Two jobs whose areas are disjoint
    /// neither read nor write a cell the other changes.
    pub fn area(&self) -> BlockBox {
        self.layout.block_bounds()
    }

    /// Everything is erased before anything is filled. Relaxation tolerates any
    /// interleaving because it only ever raises values, but erasing lowers
    /// them, and an erase racing a fill would silently drop light that nothing
    /// will resend.
    pub fn run(self) -> LightUpdate {
        let LightJob {
            registry,
            layout,
            blocks,
            mut published,
            erase_plan,
            sky_floors,
        } = self;

        let started = Instant::now();
        let mut block_field = LightField::new(layout.clone());
        let mut sky_field = LightField::new(layout.clone());
        for (section_index, light) in published.iter().enumerate() {
            if let Some((block_light, sky_light)) = light {
                block_field.fill_section(section_index, block_light);
                sky_field.fill_section(section_index, sky_light);
            }
        }

        let fill = started.elapsed();
        let started = Instant::now();

        // One independent piece of work per section, which is what makes this
        // parallel: sections share no cells, and the field takes writes through
        // a shared reference. Below the threshold the thread pool costs more
        // than the walk it would split.
        let seed_section = |section_index: usize| {
            let mut block_seeds = Vec::new();
            let mut sky_seeds = Vec::new();
            let mut erased = 0;

            // Unloaded space is neither lit nor a source; giving it light
            // would let that light escape into the loaded world next to it.
            if !blocks.section_holds_light(section_index) {
                return (block_seeds, sky_seeds, erased);
            }

            let section_base = layout.section_base(section_index);
            let section_pos = layout.section_pos(section_index);
            for local in LocalPos::all() {
                let index = section_base | local.index() as u32;
                let pos = block_in(section_pos, local);

                if erase_plan.covers(pos) {
                    erased += 1;

                    let emission = registry.emission(blocks.get(index), Layer::Block);
                    block_field.set(index, emission);
                    if !emission.is_zero() {
                        block_seeds.push(index);
                    }

                    let sky = if sky_floors.is_source(pos.x, pos.y, pos.z) {
                        LightLevel::MAX
                    } else {
                        LightLevel::ZERO
                    };
                    sky_field.set(index, sky);
                    if !sky.is_zero() {
                        sky_seeds.push(index);
                    }
                } else {
                    // Untouched cells are already at their final value and
                    // act as the fixed boundary that holds the calculation
                    // in place.
                    if !block_field.get(index).is_zero() {
                        block_seeds.push(index);
                    }
                    if !sky_field.get(index).is_zero() {
                        sky_seeds.push(index);
                    }
                }
            }

            (block_seeds, sky_seeds, erased)
        };

        let per_section: Vec<(Vec<CellIndex>, Vec<CellIndex>, usize)> =
            if layout.cell_count() >= PARALLEL_SEED_LIMIT {
                (0..layout.section_count())
                    .into_par_iter()
                    .map(seed_section)
                    .collect()
            } else {
                (0..layout.section_count()).map(seed_section).collect()
            };

        let mut erased_cells = 0;
        let mut block_seeds = Vec::with_capacity(per_section.iter().map(|s| s.0.len()).sum());
        let mut sky_seeds = Vec::with_capacity(per_section.iter().map(|s| s.1.len()).sum());
        for (block, sky, erased) in per_section {
            block_seeds.extend(block);
            sky_seeds.extend(sky);
            erased_cells += erased;
        }

        let seed = started.elapsed();
        let started = Instant::now();

        // The two layers share nothing but the blocks, so they run side by side.
        let (block_rounds, sky_rounds) = rayon::join(
            || relax(&block_field, &blocks, &registry, block_seeds),
            || relax(&sky_field, &blocks, &registry, sky_seeds),
        );

        let relax_time = started.elapsed();
        let started = Instant::now();

        let mut sections = Vec::with_capacity(layout.section_count());
        for (section_index, section_pos) in layout.sections() {
            if !blocks.section_loaded(section_index) {
                continue;
            }
            let (was_block, was_sky) = published[section_index].take().unzip();
            sections.push(SectionLight {
                pos: section_pos,
                block_light: keep_published(was_block, block_field.section_light(section_index)),
                sky_light: keep_published(was_sky, sky_field.section_light(section_index)),
            });
        }

        LightUpdate {
            sections,
            stats: EpochStats {
                block_rounds,
                sky_rounds,
                erased_cells,
                area_cells: layout.cell_count(),
                timings: EpochTimings {
                    fill,
                    seed,
                    relax: relax_time,
                    read_back: started.elapsed(),
                },
            },
        }
    }
}

impl LightWorld {
    /// Applies edits to the world and returns the lighting work they create.
    ///
    /// The block changes take effect here and now; only the *lighting* is
    /// deferrable. Queueing the change itself would mean an edit held back by a
    /// batch limit had not happened yet.
    ///
    /// Not free: an edit that moves the sky boundary rescans its column, and
    /// loading a section rescans all 256 of them.
    pub fn apply_edits(
        &mut self,
        edits: impl IntoIterator<Item = Edit>,
    ) -> Vec<(ColumnPos, Influence)> {
        let mut work = Vec::new();
        // Loading a stack of sections would otherwise rescan the same 256
        // columns once per section; deduplicating is worth an order of
        // magnitude on a bulk load.
        let mut columns_to_rescan: HashSet<ColumnPos> = HashSet::new();

        for edit in edits {
            let column = edit.column();
            let influence = match edit {
                Edit::SetBlock { pos, block } => self.edit_block(pos, block),
                Edit::LoadSection { pos, blocks } => {
                    self.insert_section(pos, Section::new(blocks));
                    columns_to_rescan.insert(column);
                    Some(Influence::new(BlockBox::of_section(pos), INFLUENCE_RADIUS))
                }
                Edit::UnloadSection { pos } => self.remove_section(pos).map(|_| {
                    columns_to_rescan.insert(column);
                    Influence::new(BlockBox::of_section(pos), INFLUENCE_RADIUS)
                }),
            };
            if let Some(influence) = influence {
                work.push((column, influence));
            }
        }

        for column in columns_to_rescan {
            if self.column_is_loaded(column) {
                if let Some(core) = self.rescan_column(column) {
                    work.push((column, Influence::new(core, INFLUENCE_RADIUS)));
                }
            } else {
                self.forget_sky_floors(column);
            }
        }

        work
    }

    /// Builds one unit of work from influences already applied to the world.
    pub fn prepare_batch(
        &mut self,
        influences: impl IntoIterator<Item = Influence>,
    ) -> Option<LightJob> {
        let mut regions = Regions::default();
        for influence in influences {
            regions.push(influence);
        }

        let influenced = regions.bounds()?;
        // One cell of margin so the calculation is surrounded by values it is
        // not allowed to change.
        let area = influenced.expand(1).clamp_vertically(self.bounds());
        let layout = FieldLayout::covering(area);

        let mut sections: Vec<SectionSource> = Vec::with_capacity(layout.section_count());
        let mut published: Vec<Option<(LightStorage, LightStorage)>> =
            Vec::with_capacity(layout.section_count());

        for (_, section_pos) in layout.sections() {
            match self.section(section_pos) {
                Some(section) => {
                    sections.push(SectionSource::Loaded(Arc::clone(&section.blocks)));
                    published.push(Some((
                        section.block_light.clone(),
                        section.sky_light.clone(),
                    )));
                }
                // Absent sections start dark whatever a reader would be told
                // about them; what differs is whether they pass light.
                None if self.is_outside_world(section_pos) => {
                    sections.push(SectionSource::Open(self.registry().outside()));
                    published.push(None);
                }
                None => {
                    sections.push(SectionSource::Absent(self.registry().unloaded()));
                    published.push(None);
                }
            }
        }

        // Collected over the whole layout, which the section grid rounds
        // outward from the influenced area.
        let sky_floors = SkyFloors::collect(layout.block_bounds(), |column| self.sky_floor(column));
        let erase_plan = regions.erase_plan(layout.cell_count() as u64, influenced);

        Some(LightJob {
            registry: Arc::clone(self.registry()),
            blocks: BlockSnapshot::new(&layout, sections),
            layout,
            published,
            erase_plan,
            sky_floors,
        })
    }

    /// Applies edits and turns all of them into one unit of work.
    pub fn prepare(&mut self, edits: impl IntoIterator<Item = Edit>) -> Option<LightJob> {
        let work = self.apply_edits(edits);
        self.prepare_batch(work.into_iter().map(|(_, influence)| influence))
    }

    /// Publishes a finished result. Sections unloaded in the meantime are dropped.
    pub fn apply(&mut self, update: LightUpdate) -> Vec<ChunkPos> {
        let mut changed = Vec::with_capacity(update.sections.len());
        for section_light in update.sections {
            let Some(section) = self.section_mut(section_light.pos) else {
                continue;
            };
            section.block_light = section_light.block_light;
            section.sky_light = section_light.sky_light;
            changed.push(section_light.pos);
        }
        changed
    }

    /// Convenience for callers that do not need the work off their thread.
    pub fn run_epoch(&mut self, edits: impl IntoIterator<Item = Edit>) -> Option<LightUpdate> {
        let job = self.prepare(edits)?;
        Some(job.run())
    }

    /// Prepares, runs and publishes in one go.
    pub fn update_now(&mut self, edits: impl IntoIterator<Item = Edit>) -> EpochStats {
        match self.run_epoch(edits) {
            Some(update) => {
                let stats = update.stats;
                self.apply(update);
                stats
            }
            None => EpochStats::default(),
        }
    }

    fn edit_block(&mut self, pos: BlockPos, block: VoxelId) -> Option<Influence> {
        let registry = Arc::clone(self.registry());
        let section = self.section_mut(ChunkPos::from(pos))?;
        let local = local_of(pos);
        if !registry.light_properties_differ(section::block_at(&section.blocks, local), block) {
            return None;
        }
        section::set_block(Arc::make_mut(&mut section.blocks), local, block);

        // The affected core is the edited cell together with the run of column
        // whose sky source flag changed. Both share a horizontal position, so
        // the two collapse into one vertical box.
        let column = BlockColumn { x: pos.x, z: pos.z };
        let old_floor = self.rescan_sky_floor(column);
        let new_floor = self.sky_floor(column);

        let mut low = pos.y;
        let mut high = pos.y;
        if old_floor != new_floor {
            // A column the world has not scanned yet reports the
            // `NO_SKY_SOURCES` sentinel; clamping degrades that to "the whole
            // column" instead of overflowing when the box is later dilated.
            let bounds = self.bounds();
            let segment_low = old_floor.min(new_floor).max(bounds.min_light_y());
            let segment_high = old_floor
                .max(new_floor)
                .saturating_sub(1)
                .min(bounds.max_light_y());
            if segment_low <= segment_high {
                low = low.min(segment_low);
                high = high.max(segment_high);
            }
        }

        let core = BlockBox {
            min: BlockPos::new(pos.x, low, pos.z),
            max: BlockPos::new(pos.x, high, pos.z),
        };
        Some(Influence::new(core, INFLUENCE_RADIUS))
    }
}
