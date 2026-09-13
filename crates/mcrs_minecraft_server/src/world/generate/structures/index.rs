use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

use bevy_math::IVec3;
use mcrs_minecraft_world::biome::climate::TargetPoint;
use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
use mcrs_minecraft_worldgen::feature::placer::BiomeMask;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::{
    CONTINENTS, DEPTH, EROSION, NoiseRouter, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen::structure::StructurePlacement;
use mcrs_minecraft_worldgen::structure::placement::{
    SpreadPlacement, excluded_in_range, fixed_biome_window, frequency_gate, ring_positions,
    scan_biome_window, select_with_removal,
};
use mcrs_minecraft_worldgen::value_provider::HeightContext;
use mcrs_minecraft_worldgen::volume::Volume;
use rustc_hash::FxHashMap;

use super::locate::{LocatePlacement, MAX_SEARCH_RADIUS, locate};
use super::site::{Site, SiteWorld, site};
use super::{DimensionStructureTables, SetId, StructureId};
use crate::world::generate::modern_carvers::climate_target_at;
use crate::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::world::generate::stages::extent;
use crate::world::generate::{base_height, heightmap_kind};
use crate::world::heightmap::HeightmapPredicates;

#[derive(Clone)]
pub enum BiomeLookup {
    MultiNoise(Arc<MultiNoiseBiomeTable>),
    Fixed(u32),
    None,
}

const CLIMATE_ROOTS: [usize; 6] = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];

struct CellSlot {
    sites: Box<[OnceLock<Option<Site>>]>,
}

type RingSets = Vec<(SetId, Vec<(i32, i32)>)>;

pub struct StructureIndex {
    tables: Arc<DimensionStructureTables>,
    seed: i64,
    router: Arc<NoiseRouter>,
    biomes: BiomeLookup,
    predicates: Option<HeightmapPredicates>,
    accessor_min_y: i32,
    accessor_height: i32,
    rings: OnceLock<RingSets>,
    ring_task: Mutex<Option<JoinHandle<RingSets>>>,
    // ponytail: the cell memo is unbounded until the staging store evicts it.
    cells: Mutex<FxHashMap<(i32, i32), Arc<CellSlot>>>,
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
        let has_rings = tables
            .live
            .iter()
            .any(|(set, _)| matches!(placement(set), StructurePlacement::ConcentricRings { .. }));
        let (rings, ring_task) = if has_rings {
            let inputs = (Arc::clone(&tables), Arc::clone(&router), biomes.clone());
            let task = std::thread::spawn(move || ring_sets(&inputs.0, seed, &inputs.1, &inputs.2));
            (OnceLock::new(), Some(task))
        } else {
            (OnceLock::from(Vec::new()), None)
        };
        StructureIndex {
            tables,
            seed,
            router,
            biomes,
            predicates,
            accessor_min_y,
            accessor_height,
            rings,
            ring_task: Mutex::new(ring_task),
            cells: Mutex::new(FxHashMap::default()),
        }
    }

    pub fn tables(&self) -> &DimensionStructureTables {
        &self.tables
    }

    pub fn rings(&self, set: SetId) -> Option<&[(i32, i32)]> {
        let rings = self.rings.get_or_init(|| {
            let task = self.ring_task.lock().unwrap().take();
            task.expect("the ring task is joined exactly once")
                .join()
                .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
        });
        rings
            .iter()
            .find(|(ring_set, _)| *ring_set == set)
            .map(|(_, positions)| positions.as_slice())
    }

    pub fn gate(&self, set: SetId, x: i32, z: i32) -> bool {
        let frozen_set = &self.tables.frozen.sets[set.0 as usize];
        let excluding = frozen_set.exclusion.map(|(other, _)| other);
        let excluded =
            |test_x, test_z| excluding.is_some_and(|other| self.gate(other, test_x, test_z));
        match &frozen_set.placement {
            StructurePlacement::RandomSpread { .. } => SpreadPlacement::of(&frozen_set.placement)
                .expect("a random_spread placement")
                .is_structure_chunk(self.seed, x, z, excluded),
            StructurePlacement::ConcentricRings { spreading, .. } => {
                self.rings(set)
                    .is_some_and(|positions| positions.contains(&(x, z)))
                    && frequency_gate(
                        self.seed,
                        spreading.salt.0,
                        spreading.frequency.0 as f32,
                        spreading.frequency_reduction_method,
                        x,
                        z,
                    )
                    && !frozen_set
                        .exclusion
                        .is_some_and(|(_, range)| excluded_in_range(range, x, z, excluded))
            }
            StructurePlacement::DimensionOrigin {} => (x, z) == (0, 0),
        }
    }

    pub fn site(&self, chunk: (i32, i32), structure: StructureId) -> Option<Site> {
        let slot = Arc::clone(self.cells.lock().unwrap().entry(chunk).or_insert_with(|| {
            Arc::new(CellSlot {
                sites: (0..self.tables.frozen.structures.len())
                    .map(|_| OnceLock::new())
                    .collect(),
            })
        }));
        slot.sites[structure.0 as usize]
            .get_or_init(|| {
                let mut view = View {
                    index: self,
                    ws: Workspace::new(),
                };
                site(
                    &self.tables.frozen,
                    structure,
                    chunk,
                    self.seed,
                    self.height_context(),
                    self.accessor_min_y,
                    self.accessor_height,
                    &mut view,
                )
            })
            .clone()
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
    pub fn selected(&self, set: SetId, chunk: (i32, i32)) -> Option<StructureId> {
        select_with_removal(
            self.seed,
            chunk.0,
            chunk.1,
            &self.tables.frozen.sets[set.0 as usize].entries,
            |structure| {
                self.site(chunk, structure)
                    .is_some_and(|site| site.biome_ok)
            },
        )
    }

    pub fn starts_present(&self, set: SetId, chunk: (i32, i32), structure: StructureId) -> bool {
        self.gate(set, chunk.0, chunk.1) && self.selected(set, chunk) == Some(structure)
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
        locate(
            self.seed,
            origin,
            MAX_SEARCH_RADIUS,
            &placements,
            |set| self.rings(set),
            |set, chunk, structure| self.starts_present(set, chunk, structure),
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
                    |x, z, fork| match biomes {
                        BiomeLookup::MultiNoise(table) => {
                            scan_biome_window(x, z, fork, |quart_x, quart_z, side| {
                                plane_admits(
                                    router, &mut ws, table, quart_x, quart_z, side, preferred,
                                )
                            })
                        }
                        BiomeLookup::Fixed(biome) => preferred
                            .contains(*biome as usize)
                            .then(|| fixed_biome_window(x, z, fork)),
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
    let volume = Volume::new(
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
        let volume = Volume::new(
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
