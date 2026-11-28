use rayon::prelude::*;

use std::sync::Arc;

use mcrs_voxel::BlockStateId;
use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};
use rustc_hash::FxHashSet;

use crate::block::LightRegistry;
use crate::field::{BlockSnapshot, CellIndex, FieldLayout, LightField, block_in};
use crate::level::{BlockColumn, LightLevel};
use crate::region::{ErasePlan, Influence, Regions, SectionErase};
use crate::relax::relax;
use crate::world::{Edit, LightWorld, Section, SkyFloor};
use mcrs_minecraft_chunk::LightStorage;
use mcrs_minecraft_core::BoundingBox;
use mcrs_voxel::LocalPos;

#[derive(Clone, Debug)]
pub struct SectionLight {
    pub pos: SectionPos,
    pub block_light: Option<LightStorage>,
    pub sky_light: Option<LightStorage>,
}

#[derive(Clone, Debug)]
pub struct PublishedLight {
    pub entity: u64,
    pub block_light: Option<LightStorage>,
    pub sky_light: Option<LightStorage>,
}

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
}

const PARALLEL_SECTION_LIMIT: usize = 1 << 18;

struct SkyFrontier {
    floor: [i32; SectionPos::AREA],
    neighbour_top: [i32; SectionPos::AREA],
    clear_above: i32,
}

impl SkyFrontier {
    fn of<'a>(
        floors: &SkyFloor,
        section_pos: SectionPos,
        neighbour: impl Fn(i32, i32) -> Option<&'a SkyFloor>,
    ) -> Self {
        let sides = [
            (-1, 0, neighbour(-1, 0)),
            (1, 0, neighbour(1, 0)),
            (0, -1, neighbour(0, -1)),
            (0, 1, neighbour(0, 1)),
        ];
        let base_x = section_pos.x << SectionPos::BITS;
        let base_z = section_pos.z << SectionPos::BITS;
        let mut floor = [0i32; SectionPos::AREA];
        let mut neighbour_top = [i32::MIN; SectionPos::AREA];
        for (index, (floor, neighbour_top)) in floor.iter_mut().zip(&mut neighbour_top).enumerate()
        {
            let lx = (index & SectionPos::MASK) as i32;
            let lz = (index >> SectionPos::BITS) as i32;
            let (x, z) = (base_x + lx, base_z + lz);
            *floor = floors.get(BlockColumn { x, z });
            for (dx, dz, side) in &sides {
                let (nx, nz) = (lx + dx, lz + dz);
                let inside = (0..SectionPos::SIZE as i32).contains(&nx)
                    && (0..SectionPos::SIZE as i32).contains(&nz);
                let across = match (inside, side) {
                    (true, _) => Some(floors),
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
                *neighbour_top = (*neighbour_top).max(top);
            }
        }
        let clear_above = floor
            .iter()
            .map(|floor| floor + 1)
            .chain(neighbour_top.iter().copied())
            .max()
            .unwrap_or(i32::MIN);
        Self {
            floor,
            neighbour_top,
            clear_above,
        }
    }

    fn clears(&self, base_y: i32) -> bool {
        base_y >= self.clear_above
    }

    fn holds(&self, local: LocalPos, y: i32) -> bool {
        let index = (local.x() as usize) | ((local.z() as usize) << SectionPos::BITS);
        let (Some(&floor), Some(&neighbour_top)) =
            (self.floor.get(index), self.neighbour_top.get(index))
        else {
            return true;
        };
        y <= floor || y < neighbour_top
    }
}

pub struct LightJob {
    registry: Arc<LightRegistry>,
    layout: FieldLayout,
    blocks: BlockSnapshot,
    published: Vec<Option<(LightStorage, LightStorage)>>,
    erase_plan: ErasePlan,
    dark: bool,
    sky_floors: Vec<Option<Arc<SkyFloor>>>,
}

impl LightJob {
    pub fn area(&self) -> BoundingBox {
        self.layout.block_bounds()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "erasing must finish before anything is filled"
    )]
    #[expect(
        clippy::excessive_nesting,
        reason = "erasing must finish before anything is filled"
    )]
    pub fn run(self) -> LightUpdate {
        let LightJob {
            registry,
            layout,
            blocks,
            published,
            erase_plan,
            sky_floors,
            dark,
        } = self;

        let fill = tracing::info_span!("light_fill").entered();
        let mut block_field = LightField::new(layout.clone());
        let mut sky_field = LightField::new(layout.clone());
        for (section_index, light) in published.iter().enumerate() {
            let Some((block_light, sky_light)) = light else {
                continue;
            };
            block_field.fill_section(section_index, block_light);
            sky_field.fill_section(section_index, sky_light);
        }

        drop(fill);
        let seed = tracing::info_span!("light_seed").entered();

        let sky_frontiers: Vec<Option<SkyFrontier>> = (0..layout.column_count())
            .into_par_iter()
            .map(|column_index| {
                let floors = sky_floors.get(column_index)?.as_deref()?;
                Some(SkyFrontier::of(
                    floors,
                    layout.section_pos(column_index),
                    |dx, dz| {
                        layout
                            .column_step(column_index, dx, dz)
                            .and_then(|neighbour| sky_floors.get(neighbour)?.as_deref())
                    },
                ))
            })
            .collect();

        let seed_section = |section_index: usize| {
            let mut block_seeds = Vec::new();
            let mut sky_seeds = Vec::new();

            if !blocks.section_holds_light(section_index) {
                return (block_seeds, sky_seeds);
            }
            let Some(source) = blocks.section(section_index) else {
                return (block_seeds, sky_seeds);
            };

            let section_base = layout.section_base(section_index);
            let section_pos = layout.section_pos(section_index);
            let column_index = layout.column_index(section_index);
            let floors = sky_floors.get(column_index).and_then(Option::as_deref);
            let sky_frontier = sky_frontiers.get(column_index).and_then(Option::as_ref);
            let erase = erase_plan.meets(BoundingBox::of_section(section_pos));

            if dark
                && erase == SectionErase::All
                && sky_frontier
                    .is_some_and(|frontier| frontier.clears(section_pos.y << SectionPos::BITS))
            {
                for local in LocalPos::all() {
                    sky_field.set(section_base | local.index() as u32, LightLevel::MAX);
                }
                return (block_seeds, sky_seeds);
            }

            let uniform_emission = source.uniform_block().map(|block| registry.emission(block));

            let capacity = match erase {
                SectionErase::None => 0,
                _ => SectionPos::VOLUME / 8,
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
                    if !dark {
                        let emission = uniform_emission
                            .unwrap_or_else(|| registry.emission(source.block_at(local)));
                        block_field.set(index, emission);
                        if !emission.is_zero() {
                            block_seeds.push(index);
                        }
                    }

                    let sky = if floors.is_some_and(|f| f.is_source(pos.x, pos.y, pos.z)) {
                        LightLevel::MAX
                    } else {
                        LightLevel::ZERO
                    };
                    sky_field.set(index, sky);
                    if !sky.is_zero() && sky_frontier.is_none_or(|f| f.holds(local, pos.y)) {
                        sky_seeds.push(index);
                    }
                } else {
                    if !dark && block_field.get(index).is_some_and(|level| !level.is_zero()) {
                        block_seeds.push(index);
                    }
                    if sky_field.get(index).is_some_and(|level| !level.is_zero())
                        && sky_frontier.is_none_or(|f| f.holds(local, pos.y))
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

        drop(seed);
        let relax_span = tracing::info_span!("light_relax").entered();

        let (block_rounds, sky_rounds) = rayon::join(
            || relax(&block_field, &blocks, &registry, block_seeds),
            || relax(&sky_field, &blocks, &registry, sky_seeds),
        );

        drop(relax_span);
        let _read_back = tracing::info_span!("light_read_back").entered();

        let read_back = |(section_index, was): (usize, Option<(LightStorage, LightStorage)>)| {
            if !blocks.section_loaded(section_index) {
                return None;
            }
            let (was_block, was_sky) = was.unzip();
            let block_light = if dark {
                was_block.is_none().then_some(LightStorage::Empty)
            } else {
                block_field.section_light_if_changed(section_index, was_block.as_ref())
            };
            let sky_light = sky_field.section_light_if_changed(section_index, was_sky.as_ref());
            if block_light.is_none() && sky_light.is_none() {
                return None;
            }
            Some(SectionLight {
                pos: layout.section_pos(section_index),
                block_light,
                sky_light,
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
            },
        }
    }
}

impl LightWorld {
    pub fn apply_edits(
        &mut self,
        edits: impl IntoIterator<Item = Edit>,
    ) -> Vec<(ColumnPos, Influence)> {
        let mut work = Vec::new();
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
                    let section = Section::new(self.registry(), entity, blocks);
                    self.insert_section(pos, section);
                    columns_to_rescan.insert(column);
                    Some(Influence::new(BoundingBox::of_section(pos)))
                }
                Edit::UnloadSection { pos } => self.remove_section(pos).map(|_| {
                    columns_to_rescan.insert(column);
                    Influence::new(BoundingBox::of_section(pos))
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

    pub fn prepare_batch(
        &mut self,
        influences: impl IntoIterator<Item = Influence>,
    ) -> Option<LightJob> {
        let mut regions = Regions::default();
        for influence in influences {
            regions.push(influence);
        }

        let influenced = regions.bounds()?;
        let area = influenced
            .inflated(1)
            .clamp_y(self.bounds().min_light_y(), self.bounds().max_light_y());
        let layout = FieldLayout::covering(area);

        let blocks = BlockSnapshot::of(
            &layout,
            &self.snapshot(layout.section_box()),
            self.registry(),
        );
        let mut published: Vec<Option<(LightStorage, LightStorage)>> =
            Vec::with_capacity(layout.section_count());
        let mut dark = self
            .registry()
            .emission(self.registry().outside())
            .is_zero();

        for (_, section_pos) in layout.sections() {
            match self.section(section_pos) {
                Some(section) => {
                    dark &= !section.emits && section.block_light == LightStorage::Empty;
                    published.push(
                        section
                            .lit
                            .then(|| (section.block_light.clone(), section.sky_light.clone())),
                    );
                }
                None => published.push(None),
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
            blocks,
            layout,
            published,
            erase_plan,
            sky_floors,
            dark,
        })
    }

    pub fn apply(&mut self, update: LightUpdate) -> Vec<PublishedLight> {
        let mut published = Vec::with_capacity(update.sections.len());
        for section_light in update.sections {
            let Some(section) = self.section_mut(section_light.pos) else {
                continue;
            };
            if let Some(light) = &section_light.block_light {
                section.block_light = light.clone();
            }
            if let Some(light) = &section_light.sky_light {
                section.sky_light = light.clone();
            }
            section.lit = true;
            published.push(PublishedLight {
                entity: section.entity,
                block_light: section_light.block_light,
                sky_light: section_light.sky_light,
            });
        }
        published
    }

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

    fn edit_block(&mut self, pos: BlockPos, block: BlockStateId) -> Option<Influence> {
        let registry = Arc::clone(self.registry());
        let section = self.section_mut(SectionPos::from(pos))?;
        let old = section.blocks.get(LocalPos::from(pos));
        if !registry.light_properties_differ(old.into(), block.into()) {
            return None;
        }
        Arc::make_mut(&mut section.blocks).set(LocalPos::from(pos), block);
        section.emits |= !registry.emission(block.into()).is_zero();
        self.forget_column_surface(ColumnPos::from(pos));

        let column = BlockColumn { x: pos.x, z: pos.z };
        let (old_floor, new_floor) = self.rescan_sky_floor(column);

        let mut low = pos.y;
        let mut high = pos.y;
        if let Some((segment_low, segment_high)) = self.bounds().sky_flip_span(old_floor, new_floor)
        {
            low = low.min(segment_low);
            high = high.max(segment_high);
        }

        let core = BoundingBox {
            min: BlockPos::new(pos.x, low, pos.z),
            max: BlockPos::new(pos.x, high, pos.z),
        };
        Some(Influence::new(core))
    }
}
