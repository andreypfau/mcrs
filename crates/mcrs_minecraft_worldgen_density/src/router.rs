use crate::aquifer::{AquiferConfig, GlobalFluid};
use crate::beta::BetaTerrainNoises;
use crate::cell::CellBounds;
use crate::column_bound::{ColumnBound, ColumnBoundScratch};
use crate::program::{Node, NodeId, Program, Workspace};
use crate::proto::{DensityFunctionHolder, ValueRange};
use crate::router_key::{self, RouterKey};
use glam::IVec3;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ROOT_NAMES: [&str; 8] = [
    "temperature",
    "vegetation",
    "continents",
    "erosion",
    "depth",
    "ridges",
    "chunk_surface_level",
    "final_density",
];

pub const TEMPERATURE: usize = 0;
pub const VEGETATION: usize = 1;
pub const CONTINENTS: usize = 2;
pub const EROSION: usize = 3;
pub const DEPTH: usize = 4;
pub const RIDGES: usize = 5;
pub const CHUNK_SURFACE_LEVEL: usize = 6;
pub const FINAL_DENSITY: usize = 7;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouterFunctions {
    pub temperature: DensityFunctionHolder,
    pub vegetation: DensityFunctionHolder,
    pub continents: DensityFunctionHolder,
    pub erosion: DensityFunctionHolder,
    pub depth: DensityFunctionHolder,
    pub ridges: DensityFunctionHolder,
    pub chunk_surface_level: DensityFunctionHolder,
    pub final_density: DensityFunctionHolder,
}

impl RouterFunctions {
    pub fn of_density(final_density: DensityFunctionHolder) -> Self {
        let zero = || DensityFunctionHolder::ZERO;
        RouterFunctions {
            temperature: zero(),
            vegetation: zero(),
            continents: zero(),
            erosion: zero(),
            depth: zero(),
            ridges: zero(),
            chunk_surface_level: zero(),
            final_density,
        }
    }

    pub fn roots(&self) -> [&DensityFunctionHolder; 8] {
        [
            &self.temperature,
            &self.vegetation,
            &self.continents,
            &self.erosion,
            &self.depth,
            &self.ridges,
            &self.chunk_surface_level,
            &self.final_density,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseSettings {
    pub min_y: i32,
    pub height: u32,
}

impl NoiseSettings {
    pub const fn new(min_y: i32, height: u32) -> Self {
        NoiseSettings { min_y, height }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aquifers {
    pub barrier: DensityFunctionHolder,
    pub exclusion: DensityFunctionHolder,
    pub fluid_level_floodedness: DensityFunctionHolder,
    pub fluid_level_spread: DensityFunctionHolder,
    pub lava: DensityFunctionHolder,
    pub surface_level: DensityFunctionHolder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugFunction {
    pub label: String,
    pub function: DensityFunctionHolder,
}

impl DebugFunction {
    pub fn new(label: &str, function: impl Into<DensityFunctionHolder>) -> Self {
        DebugFunction {
            label: label.to_string(),
            function: function.into(),
        }
    }
}

pub type SpawnTargetPoint = BTreeMap<ResourceLocationBuf, ValueRange>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub struct NoiseGeneratorSettings {
    pub noise: NoiseSettings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_block: Option<BlockState>,
    pub default_fluid: BlockState,
    pub noise_router: RouterFunctions,
    pub material_rule: ResourceLocationBuf,
    pub spawn_target: Vec<SpawnTargetPoint>,
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aquifers: Option<Aquifers>,
    pub legacy_random_source: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub debug_functions: Vec<DebugFunction>,
}

impl mcrs_minecraft_resource::RegistryValue for NoiseGeneratorSettings {
    type Registry = mcrs_minecraft_dimension::NoiseSettings;
}

impl NoiseGeneratorSettings {
    pub fn new(
        noise: NoiseSettings,
        default_fluid: BlockState,
        noise_router: RouterFunctions,
        material_rule: ResourceLocationBuf,
        sea_level: i32,
    ) -> Self {
        NoiseGeneratorSettings {
            noise,
            default_block: None,
            default_fluid,
            noise_router,
            material_rule,
            spawn_target: Vec::new(),
            sea_level,
            disable_mob_generation: false,
            aquifers: None,
            legacy_random_source: false,
            debug_functions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterBlocks {
    pub default_block: VoxelId,
    pub default_fluid: VoxelId,
    pub water: VoxelId,
    pub lava: VoxelId,
}

pub struct NoiseRouter {
    pub(crate) program: Program,
    pub sea_level: i32,
    pub noise: NoiseSettings,
    pub world_seed: u64,
    pub has_spawn_target: bool,
    pub default_block_state: VoxelId,
    pub default_fluid_state: VoxelId,
    pub water_state: VoxelId,
    pub(crate) lava_state: VoxelId,
    pub(crate) global_fluid: GlobalFluid,
    pub(crate) aquifer: Option<AquiferConfig>,
    beta: Option<Box<BetaTerrainNoises>>,
    cell_bounds: CellBounds,
    column_bound: Option<ColumnBound>,
    build: u64,
}

impl NoiseRouter {
    pub fn new(
        mut program: Program,
        settings: &NoiseGeneratorSettings,
        world_seed: u64,
        blocks: RouterBlocks,
        aquifer: Option<AquiferConfig>,
    ) -> Self {
        let cell_bounds = CellBounds::new(&program, program.root_node(FINAL_DENSITY));
        let column_bound = cell_bounds
            .chain_input()
            .and_then(|input| ColumnBound::new(&mut program, input, cell_bounds.inputs()));
        Self {
            program,
            sea_level: settings.sea_level,
            noise: settings.noise,
            world_seed,
            has_spawn_target: !settings.spawn_target.is_empty(),
            default_block_state: blocks.default_block,
            default_fluid_state: blocks.default_fluid,
            water_state: blocks.water,
            lava_state: blocks.lava,
            global_fluid: GlobalFluid::new(settings.sea_level, blocks.default_fluid, blocks.lava),
            aquifer,
            beta: settings
                .legacy_random_source
                .then(|| Box::new(BetaTerrainNoises::new(world_seed))),
            cell_bounds,
            column_bound,
            build: router_key::next_build(),
        }
    }

    pub fn beta_noises(&self) -> Option<&BetaTerrainNoises> {
        self.beta.as_deref()
    }

    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    #[must_use]
    pub fn lava_state(&self) -> VoxelId {
        self.lava_state
    }

    #[must_use]
    pub fn aquifer(&self) -> Option<&AquiferConfig> {
        self.aquifer.as_ref()
    }

    #[must_use]
    pub fn key(&self) -> RouterKey {
        RouterKey::new(self.build, self.world_seed)
    }
}

impl NoiseRouter {
    pub fn fill_roots(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        roots: &[usize],
        out: &mut [f32],
    ) {
        let n = volume.len();
        assert_eq!(out.len(), roots.len() * n, "one row per root");
        for (k, &root) in roots.iter().enumerate() {
            if let Some(row) = out.get_mut(k * n..(k + 1) * n) {
                self.program.fill(ws, volume, root, row);
            }
        }
    }

    pub fn fill_nodes(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        nodes: &[NodeId],
        out: &mut [f32],
    ) {
        let n = volume.len();
        assert_eq!(out.len(), nodes.len() * n, "one row per node");
        for (k, &node) in nodes.iter().enumerate() {
            if let Some(row) = out.get_mut(k * n..(k + 1) * n) {
                self.program.fill_node(ws, volume, node, row);
            }
        }
    }

    pub fn fill_cell_inputs(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        scratch: &ColumnBoundScratch,
        out: &mut [f32],
    ) {
        let seeded = scratch.serves(self.key())
            && self
                .column_bound
                .as_ref()
                .is_some_and(|bound| bound.fill_inputs(&self.program, ws, volume, scratch, out));
        if !seeded {
            self.fill_nodes(ws, volume, self.cell_inputs(), out);
        }
    }

    pub fn pin_cell_lattice(&self, ws: &mut Workspace, volume: &SampleGrid, values: &[f32]) {
        self.pin_lattice_of(&self.cell_bounds, ws, volume, values);
    }

    pub fn pin_lattice_of(
        &self,
        bounds: &CellBounds,
        ws: &mut Workspace,
        volume: &SampleGrid,
        values: &[f32],
    ) {
        let stride = volume.len();
        assert_eq!(
            values.len(),
            bounds.wrappers().len() * stride,
            "one row per interpolated wrapper"
        );
        for (k, &wrapper) in bounds.wrappers().iter().enumerate() {
            let (Node::Interpolated { cell, .. }, Some(row)) = (
                self.program.node(wrapper),
                values.get(k * stride..(k + 1) * stride),
            ) else {
                continue;
            };
            ws.pin_lattice(*cell, volume, row);
        }
    }

    #[inline]
    pub fn cell_inputs(&self) -> &[NodeId] {
        self.cell_bounds.inputs()
    }

    #[inline]
    pub fn cell_size(&self) -> Option<IVec3> {
        self.cell_bounds.cell_size()
    }

    pub fn final_density_chain_bound(
        &self,
        values: &[f32],
        lattice: &SampleGrid,
        at: IVec3,
        min: IVec3,
        max: IVec3,
    ) -> Option<Interval> {
        self.cell_bounds
            .chain_bound(&self.program, values, lattice, at, min, max)
    }

    pub fn final_density_first_air_row(
        &self,
        ws: &mut Workspace,
        lattice: &SampleGrid,
        hint: Option<i32>,
        scratch: &mut ColumnBoundScratch,
    ) -> i32 {
        let top = lattice.size().y - 1;
        scratch.bind(self.key());
        self.column_bound.as_ref().map_or(top, |bound| {
            bound.first_air_row(&self.program, &self.cell_bounds, ws, lattice, hint, scratch)
        })
    }

    pub fn final_density_cell_bounds(
        &self,
        corners: &[Interval],
        min: IVec3,
        max: IVec3,
        terms: &mut Vec<Interval>,
    ) -> Option<Interval> {
        self.cell_bounds
            .eval(&self.program, corners, min, max, terms)
    }

    pub fn sample_beta_climate_grids(
        &self,
        ws: &mut Workspace,
        block_x: i32,
        block_z: i32,
    ) -> ([f32; 256], [f32; 256]) {
        let volume = SampleGrid::new(
            IVec3::new(16, 1, 16),
            IVec3::new(block_x, 0, block_z),
            IVec3::ONE,
        );
        let mut values = vec![0.0f32; 2 * volume.len()];
        self.fill_roots(ws, &volume, &[TEMPERATURE, VEGETATION], &mut values);
        let mut temperature = [0.0f32; 256];
        let mut vegetation = [0.0f32; 256];
        let read = |i: usize| values.get(i).copied().unwrap_or_default();
        let columns = temperature.iter_mut().zip(vegetation.iter_mut());
        for (column, (temperature, vegetation)) in (0..16i32 * 16).zip(columns) {
            let source = volume.index_unchecked(column / 16, 0, column % 16);
            *temperature = read(source);
            *vegetation = read(volume.len() + source);
        }
        (temperature, vegetation)
    }

    pub fn sample_beta_climate(
        &self,
        ws: &mut Workspace,
        block_x: i32,
        block_z: i32,
    ) -> (f32, f32) {
        let volume = SampleGrid::point(IVec3::new(block_x, 0, block_z));
        let mut values = [0.0f32; 2];
        self.fill_roots(ws, &volume, &[TEMPERATURE, VEGETATION], &mut values);
        (values[0], values[1])
    }
}
