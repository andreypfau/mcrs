//! One batch of edits, turned into new light, split so the expensive part can
//! leave the tick loop: [`LightWorld::prepare_batch`] runs on the caller's
//! thread, [`LightJob::run`] is pure computation holding no borrows, and
//! [`LightWorld::apply`] publishes the result.

use rayon::prelude::*;

use std::sync::Arc;
use std::time::{Duration, Instant};

use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_math::{BlockPos, ChunkPos, ColumnPos};
use mcrs_voxel_storage::VoxelId;
use rustc_hash::FxHashSet;

use crate::block::LightRegistry;
use crate::field::{BlockSnapshot, CellIndex, FieldLayout, LightField, SectionSource, block_in};
use crate::level::{BlockColumn, LightLevel, LocalPos};
use crate::region::{BlockBox, ErasePlan, Influence, Regions, SectionErase};
use crate::relax::relax;
use crate::storage::LightStorage;
use crate::world::{Edit, LightWorld, Section, SkyFloor};

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
    pub area_cells: usize,
    pub timings: EpochTimings,
}

/// Below this many cells the per-section passes run on one thread: a small
/// field costs less to walk than a thread pool costs to start.
const PARALLEL_SECTION_LIMIT: usize = 1 << 18;

/// Which sky-source cells of a section column can still raise a neighbour.
///
/// A source cell sits at `LightLevel::MAX`, and so does every source cell
/// around it, so relaxing one against another can never raise anything: the
/// only source cells worth seeding are those touching a cell that is not a
/// source. Above the terrain that is a thin skin instead of the whole shaft,
/// which is most of the field.
struct SkyFrontier {
    floor: [i32; BLOCKS::AREA],
    /// The highest floor among a column's four horizontal neighbours. A source
    /// cell below it faces a non-source and has to be seeded.
    neighbour_top: [i32; BLOCKS::AREA],
}

impl SkyFrontier {
    fn of<'a>(
        floors: &SkyFloor,
        section_pos: ChunkPos,
        neighbour: impl Fn(i32, i32) -> Option<&'a SkyFloor>,
    ) -> Self {
        let sides = [
            (-1, 0, neighbour(-1, 0)),
            (1, 0, neighbour(1, 0)),
            (0, -1, neighbour(0, -1)),
            (0, 1, neighbour(0, 1)),
        ];
        let base_x = section_pos.x << BLOCKS::BITS;
        let base_z = section_pos.z << BLOCKS::BITS;
        let mut floor = [0i32; BLOCKS::AREA];
        let mut neighbour_top = [i32::MIN; BLOCKS::AREA];
        for index in 0..BLOCKS::AREA {
            let lx = (index & BLOCKS::MASK) as i32;
            let lz = (index >> BLOCKS::BITS) as i32;
            let (x, z) = (base_x + lx, base_z + lz);
            floor[index] = floors.get(BlockColumn { x, z });
            for (dx, dz, side) in &sides {
                let (nx, nz) = (lx + dx, lz + dz);
                let inside = (0..BLOCKS::SIZE as i32).contains(&nx)
                    && (0..BLOCKS::SIZE as i32).contains(&nz);
                let across = match (inside, side) {
                    (true, _) => Some(floors),
                    // Off the layout, or a column the world holds no sky scan
                    // for: assume the worst and let every source cell seed.
                    (false, None) => None,
                    (false, Some(side)) => Some(*side),
                };
                let top = match across {
                    Some(across) => across.get(BlockColumn {
                        x: x + dx,
                        z: z + dz,
                    }),
                    None => i32::MAX,
                };
                neighbour_top[index] = neighbour_top[index].max(top);
            }
        }
        Self {
            floor,
            neighbour_top,
        }
    }

    fn holds(&self, local: LocalPos, y: i32) -> bool {
        let index = (local.x() as usize) | ((local.z() as usize) << BLOCKS::BITS);
        let floor = self.floor[index];
        // Below the floor it is not a source at all: its value came from
        // propagation, and relaxation still has to start from it.
        y <= floor || y < self.neighbour_top[index]
    }
}

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
    /// One entry per section column of the layout, shared with the world rather
    /// than copied: the scan that produced them is the world's own.
    sky_floors: Vec<Option<Arc<SkyFloor>>>,
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
            published,
            erase_plan,
            sky_floors,
        } = self;

        let started = Instant::now();
        let mut block_field = LightField::new(layout.clone());
        let mut sky_field = LightField::new(layout.clone());
        for (section_index, light) in published.iter().enumerate() {
            let Some((block_light, sky_light)) = light else {
                continue;
            };
            block_field.fill_section(section_index, block_light);
            sky_field.fill_section(section_index, sky_light);
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

            // Unloaded space is neither lit nor a source; giving it light
            // would let that light escape into the loaded world next to it.
            if !blocks.section_holds_light(section_index) {
                return (block_seeds, sky_seeds);
            }

            let section_base = layout.section_base(section_index);
            let section_pos = layout.section_pos(section_index);
            let column_index = layout.column_index(section_index);
            let floors = sky_floors[column_index].as_deref();
            let sky_frontier = floors.map(|floors| {
                SkyFrontier::of(floors, section_pos, |dx, dz| {
                    layout
                        .column_step(column_index, dx, dz)
                        .and_then(|neighbour| sky_floors[neighbour].as_deref())
                })
            });
            let source = blocks.section(section_index);
            let erase = erase_plan.meets(BlockBox::of_section(section_pos));

            // Every cell of a uniform section emits the same light, and most
            // sections of a loading world are uniform air.
            let uniform_emission = source.uniform_block().map(|block| registry.emission(block));

            let capacity = match erase {
                SectionErase::None => 0,
                _ => BLOCKS::VOLUME / 8,
            };
            block_seeds.reserve(capacity);
            sky_seeds.reserve(capacity);

            for local in LocalPos::all() {
                let index = section_base | local.index() as u32;
                let pos = block_in(section_pos, local);

                let erased = match erase {
                    SectionErase::All => true,
                    SectionErase::None => false,
                    SectionErase::Partial => erase_plan.covers(pos),
                };
                if erased {
                    let emission = uniform_emission
                        .unwrap_or_else(|| registry.emission(source.block_at(local)));
                    block_field.set(index, emission);
                    if !emission.is_zero() {
                        block_seeds.push(index);
                    }

                    let sky = if floors.is_some_and(|f| f.is_source(pos.x, pos.y, pos.z)) {
                        LightLevel::MAX
                    } else {
                        LightLevel::ZERO
                    };
                    sky_field.set(index, sky);
                    if !sky.is_zero() && sky_frontier.as_ref().is_none_or(|f| f.holds(local, pos.y))
                    {
                        sky_seeds.push(index);
                    }
                } else {
                    // Untouched cells are already at their final value and
                    // act as the fixed boundary that holds the calculation
                    // in place.
                    if !block_field.get(index).is_zero() {
                        block_seeds.push(index);
                    }
                    if !sky_field.get(index).is_zero()
                        && sky_frontier.as_ref().is_none_or(|f| f.holds(local, pos.y))
                    {
                        sky_seeds.push(index);
                    }
                }
            }

            (block_seeds, sky_seeds)
        };

        let per_section: Vec<(Vec<CellIndex>, Vec<CellIndex>)> =
            if layout.cell_count() >= PARALLEL_SECTION_LIMIT {
                (0..layout.section_count())
                    .into_par_iter()
                    .map(seed_section)
                    .collect()
            } else {
                (0..layout.section_count()).map(seed_section).collect()
            };

        let (block_seeds, sky_seeds): (Vec<Vec<CellIndex>>, Vec<Vec<CellIndex>>) =
            per_section.into_iter().unzip();
        let (block_seeds, sky_seeds) = (block_seeds.concat(), sky_seeds.concat());

        let seed = started.elapsed();
        let started = Instant::now();

        // The two layers share nothing but the blocks, so they run side by side.
        let (block_rounds, sky_rounds) = rayon::join(
            || relax(&block_field, &blocks, &registry, block_seeds),
            || relax(&sky_field, &blocks, &registry, sky_seeds),
        );

        let relax_time = started.elapsed();
        let started = Instant::now();

        // Independent per section like the seeding pass, and just as expensive:
        // 4096 loads and a repack each.
        let read_back = |(section_index, was): (usize, Option<(LightStorage, LightStorage)>)| {
            let (was_block, was_sky) = was.unzip();
            blocks.section_loaded(section_index).then(|| SectionLight {
                pos: layout.section_pos(section_index),
                block_light: keep_published(was_block, block_field.section_light(section_index)),
                sky_light: keep_published(was_sky, sky_field.section_light(section_index)),
            })
        };
        let sections: Vec<SectionLight> = if layout.cell_count() >= PARALLEL_SECTION_LIMIT {
            published
                .into_par_iter()
                .enumerate()
                .filter_map(read_back)
                .collect()
        } else {
            published
                .into_iter()
                .enumerate()
                .filter_map(read_back)
                .collect()
        };

        LightUpdate {
            sections,
            stats: EpochStats {
                block_rounds,
                sky_rounds,
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
        let mut columns_to_rescan: FxHashSet<ColumnPos> = FxHashSet::default();

        for edit in edits {
            let column = edit.column();
            let influence = match edit {
                Edit::SetBlock { pos, block } => self.edit_block(pos, block),
                Edit::LoadSection {
                    pos,
                    entity,
                    blocks,
                } => {
                    self.insert_section(pos, Section::new(entity, blocks));
                    columns_to_rescan.insert(column);
                    Some(Influence::new(BlockBox::of_section(pos)))
                }
                Edit::UnloadSection { pos } => self.remove_section(pos).map(|_| {
                    columns_to_rescan.insert(column);
                    Influence::new(BlockBox::of_section(pos))
                }),
                Edit::SetColumnSurface { column, surface } => {
                    self.set_column_surface(column, surface);
                    None
                }
            };
            if let Some(influence) = influence {
                work.push((column, influence));
            }
        }

        for column in columns_to_rescan {
            if self.column_is_loaded(column) {
                if let Some(core) = self.rescan_column(column) {
                    work.push((column, Influence::new(core)));
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

        let sky_floors = (0..layout.column_count())
            .map(|index| {
                let pos = layout.section_pos(index);
                self.sky_floors_of(ColumnPos::new(pos.x, pos.z))
            })
            .collect();
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

    /// Applies edits, lights them and publishes the result on this thread.
    pub fn update_now(&mut self, edits: impl IntoIterator<Item = Edit>) -> EpochStats {
        let work = self.apply_edits(edits);
        let Some(job) = self.prepare_batch(work.into_iter().map(|(_, influence)| influence)) else {
            return EpochStats::default();
        };
        let update = job.run();
        let stats = update.stats;
        self.apply(update);
        stats
    }

    fn edit_block(&mut self, pos: BlockPos, block: VoxelId) -> Option<Influence> {
        let registry = Arc::clone(self.registry());
        let section = self.section_mut(ChunkPos::from(pos))?;
        if !registry.light_properties_differ(section.blocks.get(pos), block) {
            return None;
        }
        Arc::make_mut(&mut section.blocks).set(pos, block);
        // Past the early return above, so a write the scan cannot tell apart
        // from what it replaced keeps the bound: equal light properties give
        // equal seams, whatever the surface now says about that block.
        self.forget_column_surface(ColumnPos::from(pos));

        // The affected core is the edited cell together with the run of column
        // whose sky source flag changed. Both share a horizontal position, so
        // the two collapse into one vertical box.
        let column = BlockColumn { x: pos.x, z: pos.z };
        let (old_floor, new_floor) = self.rescan_sky_floor(column);

        let mut low = pos.y;
        let mut high = pos.y;
        if let Some((segment_low, segment_high)) = self.bounds().sky_flip_span(old_floor, new_floor)
        {
            low = low.min(segment_low);
            high = high.max(segment_high);
        }

        let core = BlockBox {
            min: BlockPos::new(pos.x, low, pos.z),
            max: BlockPos::new(pos.x, high, pos.z),
        };
        Some(Influence::new(core))
    }
}
