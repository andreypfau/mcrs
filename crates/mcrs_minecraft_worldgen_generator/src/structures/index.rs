use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

use bevy_math::IVec3;
use mcrs_minecraft_biome::climate::TargetPoint;
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_worldgen::beard::{Beard, BeardPiece, JunctionPoint};
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    CONTINENTS, DEPTH, EROSION, NoiseRouter, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::BiomeMask;
use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_structure::StructurePlacement;
use mcrs_minecraft_worldgen_structure::placement::{
    SpreadPlacement, excluded_in_range, fixed_biome_window, frequency_gate, ring_positions,
    scan_biome_window, select_with_removal,
};

use crate::heightmap::HeightmapPredicates;
use crate::modern_carvers::climate_target_at;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::stages::extent;
use crate::{base_height, heightmap_kind};
use mcrs_minecraft_worldgen_structure::frozen::{
    DimensionStructureTables, SetId, StructureId, StructureKind,
};
use mcrs_minecraft_worldgen_structure::jigsaw::{Piece, Start, layout};
use mcrs_minecraft_worldgen_structure::locate::{LocatePlacement, MAX_SEARCH_RADIUS, locate};
use mcrs_minecraft_worldgen_structure::site::{Site, SiteWorld, site};

#[derive(Clone)]
pub enum BiomeLookup {
    MultiNoise(Arc<MultiNoiseBiomeTable>),
    Fixed(u32),
    None,
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
    accessor_min_y: i32,
    accessor_height: i32,
    rings: RingSets,
    // ponytail: unbounded; only gate-passing chunks enter, but the memo grows
    // with the explored area until the staging store's wanted set evicts it.
    starts: Mutex<HashMap<(SetId, ColumnPos), StartCell>>,
}

impl StructureIndex {
    pub fn new(
        tables: Arc<DimensionStructureTables>,
        seed: i64,
        router: Arc<NoiseRouter>,
        biomes: BiomeLookup,
        predicates: Option<HeightmapPredicates>,
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
        site(
            &self.tables.frozen,
            structure,
            chunk,
            self.seed,
            self.height_context(),
            self.accessor_min_y,
            self.accessor_height,
            view,
        )
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
        let (structure, site) = self.selected_site(view, set, chunk)?;
        let pieces = layout(
            frozen,
            structure,
            site,
            self.accessor_min_y,
            self.accessor_height,
            view,
        );
        (!pieces.is_empty()).then(|| {
            Start::new(
                frozen,
                structure,
                pieces.into_iter().map(Piece::Jigsaw).collect(),
            )
        })
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
        let kept = |id: StructureId| {
            matches!(
                frozen.structures[id.0 as usize].kind,
                StructureKind::Jigsaw { .. }
            )
        };
        let sets: Vec<SetId> = self
            .tables
            .live
            .iter()
            .filter(|(_, structures)| structures.iter().any(|id| kept(*id)))
            .map(|(set, _)| *set)
            .collect();
        if sets.is_empty() {
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
                    self.starts_of(&mut view, chunk, sets.iter().copied())
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
                let Piece::Jigsaw(piece) = piece;
                let junctions = piece.junctions.iter().map(|junction| JunctionPoint {
                    x: junction.source_x,
                    ground_y: junction.source_ground_y,
                    z: junction.source_z,
                });
                (
                    adaptation,
                    BeardPiece {
                        bounds: piece.bounds,
                        projection: piece.projection,
                        ground_level_delta: piece.ground_level_delta,
                        junctions,
                    },
                )
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

    // ponytail: a hardcoded structure has no site and so never selects; a set
    // mixing one with jigsaw entries picks the jigsaw entry where vanilla would
    // have placed the hardcoded one, until those generators exist.
    pub fn selected(&self, set: SetId, chunk: ColumnPos) -> Option<StructureId> {
        self.selected_site(&mut self.view(), set, chunk)
            .map(|(structure, _)| structure)
    }

    fn selected_site(
        &self,
        view: &mut View<'_>,
        set: SetId,
        chunk: ColumnPos,
    ) -> Option<(StructureId, Site)> {
        let mut accepted = None;
        let structure = select_with_removal(
            self.seed,
            chunk,
            &self.tables.frozen.sets[set.0 as usize].entries,
            |structure| {
                accepted = self
                    .site_in(view, chunk, structure)
                    .filter(|site| site.biome_ok);
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
    let mut ws = Workspace::new();
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
            let positions =
                ring_positions(
                    seed,
                    distance.0,
                    spread.0,
                    count.0,
                    |initial, fork| match biomes {
                        BiomeLookup::MultiNoise(table) => {
                            scan_biome_window(initial, fork, |quart_x, quart_z, side| {
                                plane_admits(
                                    router, &mut ws, table, quart_x, quart_z, side, preferred,
                                )
                            })
                        }
                        BiomeLookup::Fixed(biome) => preferred
                            .contains(*biome as usize)
                            .then(|| fixed_biome_window(initial, fork)),
                        BiomeLookup::None => None,
                    },
                );
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

struct View<'a> {
    index: &'a StructureIndex,
    ws: Workspace,
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
            BiomeLookup::None => None,
        }
    }

    fn column_admits(&mut self, x: i32, z: i32, biomes: &BiomeMask) -> bool {
        let table = match &self.index.biomes {
            BiomeLookup::MultiNoise(table) => table,
            BiomeLookup::Fixed(biome) => return biomes.contains(*biome as usize),
            BiomeLookup::None => return false,
        };
        let min_quart_y = self.index.accessor_min_y >> 2;
        let max_quart_y = (self.index.accessor_min_y + self.index.accessor_height - 1) >> 2;
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
}
