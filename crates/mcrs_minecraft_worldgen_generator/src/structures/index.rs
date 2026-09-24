use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

use bevy_math::IVec3;
use mcrs_minecraft_biome::climate::TargetPoint;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen::beard::{Beard, BeardPiece, JunctionPoint};
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    CONTINENTS, DEPTH, EROSION, NoiseRouter, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{BiomeMask, WorldStates};
use mcrs_minecraft_worldgen_feature::template::Projection;
use mcrs_minecraft_worldgen_feature_place::terrain_skin::{
    BiomeClimate, RAIN_TEMPERATURE, temperature,
};
use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_structure::StructurePlacement;
use mcrs_minecraft_worldgen_structure::placement::{
    SpreadPlacement, excluded_in_range, fixed_biome_window, frequency_gate, ring_starts,
    scan_biome_window, select_with_removal,
};
use rayon::prelude::*;

use crate::heightmap::HeightmapPredicates;
use crate::modern_carvers::climate_target_at;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::stages::extent;
use crate::{base_column, base_height, heightmap_kind};
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, SetId, StructureId};
use mcrs_minecraft_worldgen_structure::locate::{LocatePlacement, MAX_SEARCH_RADIUS, locate};
use mcrs_minecraft_worldgen_structure::piece::{Piece, Start};
use mcrs_minecraft_worldgen_structure::site::{
    BaseColumn, Context, Site, SiteWorld, layout, site, site_implies_piece,
};

#[derive(Clone)]
pub enum BiomeLookup {
    MultiNoise(Arc<MultiNoiseBiomeTable>),
    Fixed(u32),
    TheEnd(EndBiomes),
    None,
}

/// `TheEndBiomeSource`: the five biomes it draws from, by registry id.
#[derive(Clone, Copy, Debug)]
pub struct EndBiomes {
    pub end: u32,
    pub highlands: u32,
    pub midlands: u32,
    pub islands: u32,
    pub barrens: u32,
}

impl EndBiomes {
    pub fn resolve(mut id_of: impl FnMut(&str) -> Option<u32>) -> Option<Self> {
        Some(EndBiomes {
            end: id_of("minecraft:the_end")?,
            highlands: id_of("minecraft:end_highlands")?,
            midlands: id_of("minecraft:end_midlands")?,
            islands: id_of("minecraft:small_end_islands")?,
            barrens: id_of("minecraft:end_barrens")?,
        })
    }

    /// `getNoiseBiome`: the central island within 64 chunks of the origin,
    /// elsewhere the erosion at the chunk's centre column.
    fn at(&self, router: &NoiseRouter, ws: &mut Workspace, quart: IVec3) -> u32 {
        let block: IVec3 = quart << 2;
        let chunk_x = block.x >> 4;
        let chunk_z = block.z >> 4;
        if i64::from(chunk_x).pow(2) + i64::from(chunk_z).pow(2) <= 4096 {
            return self.end;
        }
        let volume = SampleGrid::new(
            IVec3::ONE,
            IVec3::new((chunk_x * 2 + 1) * 8, block.y, (chunk_z * 2 + 1) * 8),
            IVec3::ONE,
        );
        let mut erosion = [0.0f32];
        router.fill_roots(ws, &volume, &[EROSION], &mut erosion);
        let erosion = f64::from(erosion[0]);
        if erosion > 0.25 {
            self.highlands
        } else if erosion >= -0.0625 {
            self.midlands
        } else if erosion < -0.21875 {
            self.islands
        } else {
            self.barrens
        }
    }
}

const CLIMATE_ROOTS: [usize; 6] = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];

const MAX_STRUCTURE_DISTANCE: i32 = 8;

type RingSets = Vec<(SetId, Vec<ColumnPos>)>;
type StartCell = Arc<OnceLock<Option<Start>>>;

pub struct StructureIndex {
    tables: Arc<DimensionStructureTables>,
    seed: i64,
    router: Arc<NoiseRouter>,
    biomes: BiomeLookup,
    predicates: Option<HeightmapPredicates>,
    world: Arc<WorldStates>,
    climate: Arc<[BiomeClimate]>,
    accessor_min_y: i32,
    accessor_height: i32,
    rings: RingSets,
    // ponytail: unbounded; only gate-passing chunks enter, but the memo grows
    // with the explored area until the staging store's wanted set evicts it.
    starts: Mutex<HashMap<(SetId, ColumnPos), StartCell>>,
}

impl StructureIndex {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tables: Arc<DimensionStructureTables>,
        seed: i64,
        router: Arc<NoiseRouter>,
        biomes: BiomeLookup,
        predicates: Option<HeightmapPredicates>,
        world: Arc<WorldStates>,
        climate: Arc<[BiomeClimate]>,
        accessor_min_y: i32,
        accessor_height: i32,
    ) -> Self {
        let placement = |set: &SetId| &tables.frozen.sets[set.0 as usize].placement;
        if router.has_spawn_target
            && let Some((set, _)) = tables
                .live
                .iter()
                .find(|(set, _)| matches!(placement(set), StructurePlacement::DimensionOrigin {}))
        {
            panic!(
                "{}: dimension_origin places at the spawn chunk when the noise settings name a spawn_target, and there is no spawn finder yet",
                tables.frozen.sets[set.0 as usize].id
            );
        }
        let rings = ring_sets(&tables, seed, &router, &biomes);
        StructureIndex {
            tables,
            seed,
            router,
            biomes,
            predicates,
            world,
            climate,
            accessor_min_y,
            accessor_height,
            rings,
            starts: Mutex::default(),
        }
    }

    pub fn tables(&self) -> &DimensionStructureTables {
        &self.tables
    }

    pub fn rings(&self, set: SetId) -> Option<&[ColumnPos]> {
        self.rings
            .iter()
            .find(|(ring_set, _)| *ring_set == set)
            .map(|(_, positions)| positions.as_slice())
    }

    pub fn gate(&self, set: SetId, pos: ColumnPos) -> bool {
        let frozen_set = &self.tables.frozen.sets[set.0 as usize];
        let excluding = frozen_set.exclusion.map(|(other, _)| other);
        let excluded = |test| excluding.is_some_and(|other| self.gate(other, test));
        match &frozen_set.placement {
            StructurePlacement::RandomSpread { .. } => SpreadPlacement::of(&frozen_set.placement)
                .expect("a random_spread placement")
                .is_structure_chunk(self.seed, pos, excluded),
            StructurePlacement::ConcentricRings { spreading, .. } => {
                self.rings(set)
                    .is_some_and(|positions| positions.contains(&pos))
                    && frequency_gate(
                        self.seed,
                        spreading.salt.0,
                        spreading.frequency.0 as f32,
                        spreading.frequency_reduction_method,
                        pos,
                    )
                    && !frozen_set
                        .exclusion
                        .is_some_and(|(_, range)| excluded_in_range(range, pos, excluded))
            }
            StructurePlacement::DimensionOrigin {} => pos == ColumnPos::new(0, 0),
        }
    }

    fn view(&self) -> View<'_> {
        View {
            index: self,
            ws: Workspace::new(),
            columns: HashMap::new(),
        }
    }

    fn context<'a>(
        &'a self,
        view: &'a mut View<'_>,
        chunk: ColumnPos,
        structure: StructureId,
    ) -> Context<'a> {
        Context {
            frozen: &self.tables.frozen,
            structure: &self.tables.frozen.structures[structure.0 as usize],
            chunk,
            seed: self.seed,
            height: self.height_context(),
            accessor_min_y: self.accessor_min_y,
            accessor_height: self.accessor_height,
            world: view,
        }
    }

    pub fn site(&self, chunk: ColumnPos, structure: StructureId) -> Option<Site> {
        self.site_in(&mut self.view(), chunk, structure)
    }

    fn site_in(
        &self,
        view: &mut View<'_>,
        chunk: ColumnPos,
        structure: StructureId,
    ) -> Option<Site> {
        site(&mut self.context(view, chunk, structure))
    }

    /// `Structure.generate` for one structure at one chunk, whatever its
    /// set's draw would select there.
    pub fn start_of(&self, chunk: ColumnPos, structure: StructureId) -> Option<Start> {
        let mut view = self.view();
        let site = self
            .site_in(&mut view, chunk, structure)
            .filter(|site| site.biome_ok)?;
        let pieces = layout(&mut self.context(&mut view, chunk, structure), site);
        (!pieces.is_empty()).then(|| Start::new(&self.tables.frozen, structure, pieces))
    }

    pub fn starts_at(&self, chunk: ColumnPos) -> Vec<Start> {
        let sets = self.tables.live.iter().map(|(set, _)| *set);
        self.starts_of(&mut self.view(), chunk, sets)
    }

    fn starts_of(
        &self,
        view: &mut View<'_>,
        chunk: ColumnPos,
        sets: impl Iterator<Item = SetId>,
    ) -> Vec<Start> {
        sets.filter(|set| self.gate(*set, chunk))
            .filter_map(|set| {
                let cell = Arc::clone(
                    self.starts
                        .lock()
                        .expect("the start memo is never poisoned")
                        .entry((set, chunk))
                        .or_default(),
                );
                cell.get_or_init(|| self.start_in(view, set, chunk)).clone()
            })
            .collect()
    }

    fn start_in(&self, view: &mut View<'_>, set: SetId, chunk: ColumnPos) -> Option<Start> {
        let frozen = &self.tables.frozen;
        let (structure, selected) = self.selected_site(view, set, chunk)?;
        let pieces = match selected {
            Selected::Pieces(pieces) => pieces,
            Selected::Site(site) => layout(&mut self.context(view, chunk, structure), site),
        };
        (!pieces.is_empty()).then(|| Start::new(frozen, structure, pieces))
    }

    /// Whether any live structure places in one of `steps`.
    pub fn places_in(&self, steps: &Range<usize>) -> bool {
        let frozen = &self.tables.frozen;
        self.tables.live.iter().any(|(_, structures)| {
            structures
                .iter()
                .any(|id| steps.contains(&(frozen.structures[id.0 as usize].step as usize)))
        })
    }

    /// Every start whose bounds cross `column`, from every chunk within
    /// [`MAX_STRUCTURE_DISTANCE`] of it, in `(step, step_index, chunk.x, chunk.z)`
    /// order.
    // ponytail: starts of one structure are ordered by chunk, where the
    // reference walks a `LongOpenHashSet`; only a column two starts of one
    // structure both cross can tell the difference.
    pub fn starts_reaching(&self, column: ColumnPos) -> Vec<(ColumnPos, Start)> {
        let frozen = &self.tables.frozen;
        if self.tables.live.is_empty() {
            return Vec::new();
        }
        let footprint = BoundingBox {
            min: BlockPos::new(column.x * 16, i32::MIN, column.z * 16),
            max: BlockPos::new(column.x * 16 + 15, i32::MAX, column.z * 16 + 15),
        };
        let mut view = self.view();
        let mut starts: Vec<(ColumnPos, Start)> = Vec::new();
        let radius = MAX_STRUCTURE_DISTANCE;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let chunk = ColumnPos::new(column.x + dx, column.z + dz);
                starts.extend(
                    self.starts_of(
                        &mut view,
                        chunk,
                        self.tables.live.iter().map(|(set, _)| *set),
                    )
                    .into_iter()
                    .filter(|start| start.bounds.intersects(footprint))
                    .map(|start| (chunk, start)),
                );
            }
        }
        starts.sort_by_key(|(chunk, start)| {
            let structure = &frozen.structures[start.structure.0 as usize];
            (structure.step, structure.step_index, chunk.x, chunk.z)
        });
        starts
    }

    /// The terrain adaptation term of `column`, or `None` where it is zero
    /// throughout.
    pub fn beard(&self, column: ColumnPos) -> Option<Beard> {
        let frozen = &self.tables.frozen;
        let starts = self.starts_reaching(column);
        let pieces = starts.iter().flat_map(|(_, start)| {
            let adaptation = frozen.structures[start.structure.0 as usize].adaptation;
            start.pieces.iter().map(move |piece| {
                let beard_piece = match piece {
                    Piece::Jigsaw(piece) => BeardPiece {
                        bounds: piece.bounds,
                        projection: piece.projection,
                        ground_level_delta: piece.ground_level_delta,
                        junctions: piece
                            .junctions
                            .iter()
                            .map(|junction| JunctionPoint {
                                x: junction.source_x,
                                ground_y: junction.source_ground_y,
                                z: junction.source_z,
                            })
                            .collect::<Vec<_>>(),
                    },
                    Piece::DesertPyramid(_)
                    | Piece::JungleTemple(_)
                    | Piece::SwampHut(_)
                    | Piece::BuriedTreasure(_)
                    | Piece::Fortress(_)
                    | Piece::Shipwreck(_)
                    | Piece::OceanRuin(_)
                    | Piece::RuinedPortal(_)
                    | Piece::OceanMonument(_)
                    | Piece::Mineshaft(_)
                    | Piece::Igloo(_)
                    | Piece::NetherFossil(_)
                    | Piece::Stronghold(_)
                    | Piece::EndCity(_)
                    | Piece::WoodlandMansion(_) => BeardPiece {
                        bounds: piece.bounds(),
                        projection: Projection::Rigid,
                        ground_level_delta: 0,
                        junctions: Vec::new(),
                    },
                };
                (adaptation, beard_piece)
            })
        });
        let beard = Beard::collect(column, pieces);
        beard.affected.is_some().then_some(beard)
    }

    fn height_context(&self) -> HeightContext {
        let noise = extent(&self.router);
        let min_y = noise.min_y.max(self.accessor_min_y);
        HeightContext {
            min_y,
            depth: noise.depth.min(self.accessor_height),
            sea_level: noise.sea_level,
        }
    }

    pub fn selected(&self, set: SetId, chunk: ColumnPos) -> Option<StructureId> {
        self.selected_site(&mut self.view(), set, chunk)
            .map(|(structure, _)| structure)
    }

    /// `tryGenerateStructure` over the set's draw with removal: a site that
    /// passes its biome test, and a layout with a piece where the site alone
    /// cannot promise one.
    fn selected_site(
        &self,
        view: &mut View<'_>,
        set: SetId,
        chunk: ColumnPos,
    ) -> Option<(StructureId, Selected)> {
        let frozen = &self.tables.frozen;
        let mut accepted = None;
        let structure = select_with_removal(
            self.seed,
            chunk,
            &frozen.sets[set.0 as usize].entries,
            |structure| {
                let Some(site) = self
                    .site_in(view, chunk, structure)
                    .filter(|site| site.biome_ok)
                else {
                    return false;
                };
                let kind = &frozen.structures[structure.0 as usize].kind;
                accepted = if site_implies_piece(kind) == Some(true) {
                    Some(Selected::Site(site))
                } else {
                    let pieces = layout(&mut self.context(view, chunk, structure), site);
                    (!pieces.is_empty()).then_some(Selected::Pieces(pieces))
                };
                accepted.is_some()
            },
        )?;
        Some((structure, accepted?))
    }

    pub fn starts_present(&self, set: SetId, chunk: ColumnPos, structure: StructureId) -> bool {
        self.gate(set, chunk) && self.selected(set, chunk) == Some(structure)
    }

    pub fn locate(&self, origin: IVec3, wanted: &[StructureId]) -> Option<(IVec3, StructureId)> {
        let mut placements: Vec<LocatePlacement<'_>> = Vec::new();
        for &structure in wanted {
            for (set, structures) in &self.tables.live {
                if !structures.contains(&structure) {
                    continue;
                }
                match placements.iter_mut().find(|entry| entry.set == *set) {
                    Some(entry) => {
                        if !entry.structures.contains(&structure) {
                            entry.structures.push(structure);
                        }
                    }
                    None => placements.push(LocatePlacement {
                        set: *set,
                        placement: &self.tables.frozen.sets[set.0 as usize].placement,
                        structures: vec![structure],
                    }),
                }
            }
        }
        let mut view = self.view();
        locate(
            self.seed,
            origin,
            MAX_SEARCH_RADIUS,
            &placements,
            |set| self.rings(set),
            |set, chunk, structure| {
                self.gate(set, chunk)
                    && self
                        .selected_site(&mut view, set, chunk)
                        .is_some_and(|(selected, _)| selected == structure)
            },
        )
    }
}

fn ring_sets(
    tables: &DimensionStructureTables,
    seed: i64,
    router: &NoiseRouter,
    biomes: &BiomeLookup,
) -> RingSets {
    let frozen = &tables.frozen;
    tables
        .live
        .iter()
        .filter_map(|(set, _)| {
            let frozen_set = &frozen.sets[set.0 as usize];
            let StructurePlacement::ConcentricRings {
                distance,
                spread,
                count,
                ..
            } = &frozen_set.placement
            else {
                return None;
            };
            let preferred = frozen_set
                .preferred_biomes
                .as_ref()
                .expect("the freeze masks the preferred biomes of every ring set");
            let find =
                |ws: &mut Workspace, initial: ColumnPos, fork: &mut LegacyRandom| match biomes {
                    BiomeLookup::MultiNoise(table) => {
                        scan_biome_window(initial, fork, |quart_x, quart_z, side| {
                            plane_admits(router, ws, table, quart_x, quart_z, side, preferred)
                        })
                    }
                    BiomeLookup::Fixed(biome) => preferred
                        .contains(*biome as usize)
                        .then(|| fixed_biome_window(initial, fork)),
                    BiomeLookup::TheEnd(end) => {
                        scan_biome_window(initial, fork, |quart_x, quart_z, side| {
                            (0..side * side)
                                .map(|at| {
                                    let quart =
                                        IVec3::new(quart_x + at % side, 0, quart_z + at / side);
                                    preferred.contains(end.at(router, ws, quart) as usize)
                                })
                                .collect()
                        })
                    }
                    BiomeLookup::None => None,
                };
            let positions = ring_starts(seed, distance.0, spread.0, count.0)
                .into_par_iter()
                .map_init(Workspace::new, |ws, (initial, mut fork)| {
                    find(ws, initial, &mut fork).unwrap_or(initial)
                })
                .collect();
            Some((*set, positions))
        })
        .collect()
}

fn plane_admits(
    router: &NoiseRouter,
    ws: &mut Workspace,
    table: &MultiNoiseBiomeTable,
    quart_x: i32,
    quart_z: i32,
    side: i32,
    preferred: &BiomeMask,
) -> Vec<bool> {
    let cells = (side * side) as usize;
    let volume = SampleGrid::new(
        IVec3::new(side, 1, side),
        IVec3::new(quart_x * 4, 0, quart_z * 4),
        IVec3::splat(4),
    );
    let mut values = vec![0.0f32; CLIMATE_ROOTS.len() * cells];
    router.fill_roots(ws, &volume, &CLIMATE_ROOTS, &mut values);
    let mut last = None;
    (0..cells)
        .map(|at| {
            let target = TargetPoint::new(
                values[at],
                values[cells + at],
                values[2 * cells + at],
                values[3 * cells + at],
                values[4 * cells + at],
                values[5 * cells + at],
            );
            preferred.contains(table.biome_at_from(target, &mut last) as usize)
        })
        .collect()
}

enum Selected {
    Site(Site),
    Pieces(Vec<Piece>),
}

struct View<'a> {
    index: &'a StructureIndex,
    ws: Workspace,
    columns: HashMap<(i32, i32), Vec<VoxelId>>,
}

impl SiteWorld for View<'_> {
    fn biome_at(&mut self, block: IVec3) -> Option<u32> {
        match &self.index.biomes {
            BiomeLookup::MultiNoise(table) => {
                let target = climate_target_at(
                    &self.index.router,
                    &mut self.ws,
                    block.x >> 2,
                    block.y >> 2,
                    block.z >> 2,
                );
                Some(u32::from(table.biome_at(target)))
            }
            BiomeLookup::Fixed(biome) => Some(*biome),
            BiomeLookup::TheEnd(end) => Some(end.at(&self.index.router, &mut self.ws, block >> 2)),
            BiomeLookup::None => None,
        }
    }

    fn column_admits(
        &mut self,
        x: i32,
        z: i32,
        min_block_y: i32,
        max_block_y: i32,
        biomes: &BiomeMask,
    ) -> bool {
        let min_quart_y = min_block_y >> 2;
        let max_quart_y = max_block_y >> 2;
        let table = match &self.index.biomes {
            BiomeLookup::MultiNoise(table) => table,
            BiomeLookup::Fixed(biome) => return biomes.contains(*biome as usize),
            BiomeLookup::TheEnd(end) => {
                return (min_quart_y..=max_quart_y).any(|quart_y| {
                    let quart = IVec3::new(x >> 2, quart_y, z >> 2);
                    biomes.contains(end.at(&self.index.router, &mut self.ws, quart) as usize)
                });
            }
            BiomeLookup::None => return false,
        };
        let cells = (max_quart_y - min_quart_y + 1) as usize;
        let volume = SampleGrid::new(
            IVec3::new(1, cells as i32, 1),
            IVec3::new((x >> 2) * 4, min_quart_y * 4, (z >> 2) * 4),
            IVec3::splat(4),
        );
        let mut values = vec![0.0f32; CLIMATE_ROOTS.len() * cells];
        self.index
            .router
            .fill_roots(&mut self.ws, &volume, &CLIMATE_ROOTS, &mut values);
        (0..cells).any(|at| {
            let target = TargetPoint::new(
                values[at],
                values[cells + at],
                values[2 * cells + at],
                values[3 * cells + at],
                values[4 * cells + at],
                values[5 * cells + at],
            );
            biomes.contains(table.biome_at(target) as usize)
        })
    }

    fn all_biomes_within(&mut self, centre: IVec3, radius: i32, biomes: &BiomeMask) -> bool {
        let lo: IVec3 = (centre - IVec3::splat(radius)) >> 2;
        let hi: IVec3 = (centre + IVec3::splat(radius)) >> 2;
        let table = match &self.index.biomes {
            BiomeLookup::MultiNoise(table) => table,
            BiomeLookup::Fixed(biome) => return biomes.contains(*biome as usize),
            BiomeLookup::TheEnd(end) => {
                return (lo.x..=hi.x).all(|x| {
                    (lo.y..=hi.y).all(|y| {
                        (lo.z..=hi.z).all(|z| {
                            let quart = IVec3::new(x, y, z);
                            biomes
                                .contains(end.at(&self.index.router, &mut self.ws, quart) as usize)
                        })
                    })
                });
            }
            BiomeLookup::None => return false,
        };
        let size = hi - lo + IVec3::ONE;
        let cells = (size.x * size.y * size.z) as usize;
        let volume = SampleGrid::new(size, lo * 4, IVec3::splat(4));
        let mut values = vec![0.0f32; CLIMATE_ROOTS.len() * cells];
        self.index
            .router
            .fill_roots(&mut self.ws, &volume, &CLIMATE_ROOTS, &mut values);
        (0..cells).all(|at| {
            let target = TargetPoint::new(
                values[at],
                values[cells + at],
                values[2 * cells + at],
                values[3 * cells + at],
                values[4 * cells + at],
                values[5 * cells + at],
            );
            biomes.contains(table.biome_at(target) as usize)
        })
    }

    fn free_height(&mut self, x: i32, z: i32, heightmap: HeightmapName) -> i32 {
        let predicates = self
            .index
            .predicates
            .as_ref()
            .expect("a structure projected to a heightmap needs the heightmap predicates");
        base_height(
            &self.index.router,
            &mut self.ws,
            predicates,
            heightmap_kind(heightmap),
            x,
            z,
            self.index.accessor_min_y,
            self.index.accessor_height,
        )
    }

    fn base_column(&mut self, x: i32, z: i32) -> BaseColumn<'_> {
        let index = self.index;
        let min_y = index.router.noise.min_y.max(index.accessor_min_y);
        let states = self.columns.entry((x, z)).or_insert_with(|| {
            base_column(
                &index.router,
                &mut self.ws,
                x,
                z,
                index.accessor_min_y,
                index.accessor_height,
            )
            .1
        });
        BaseColumn {
            min_y,
            states,
            air: index.world.air,
        }
    }

    fn opaque(&self, state: VoxelId, heightmap: HeightmapName) -> bool {
        self.index
            .predicates
            .as_ref()
            .expect("a structure testing the ground needs the heightmap predicates")
            .get(state)
            .contains(heightmap_kind(heightmap))
    }

    fn states(&self) -> &WorldStates {
        &self.index.world
    }

    fn cold_enough_to_snow(&mut self, pos: IVec3, sea_level: i32) -> bool {
        let Some(biome) = self.biome_at(pos) else {
            return false;
        };
        let climate = self
            .index
            .climate
            .get(biome as usize)
            .expect("a structure asking the climate needs the climate table");
        temperature(climate, pos.into(), sea_level) < RAIN_TEMPERATURE
    }
}
