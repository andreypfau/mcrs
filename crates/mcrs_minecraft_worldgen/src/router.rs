use crate::beta::seed::BetaTerrainNoises;
use crate::cell::CellBounds;
use crate::compile::CompileError;
use crate::interval::Interval;
use crate::material::compile::MaterialProgram;
use crate::program::{Node, NodeId, Program, Workspace};
use crate::proto::{BlockState, DensityFunctionHolder, HashableF64, ValueRange};
use crate::volume::Volume;
use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_voxel_storage::VoxelId;
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
    /// In the order of [`ROOT_NAMES`], which is also the order the compiler
    /// visits them and therefore the order the graph is grown in.
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseSettings {
    pub min_y: i32,
    pub height: u32,
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

pub type SpawnTargetPoint = BTreeMap<ResourceLocation, ValueRange<HashableF64>>;

/// `worldgen/noise_settings/*.json` in full.
///
/// The density engine reads only `noise`, `noise_router`, `sea_level` and
/// `legacy_random_source`, but the shape is closed: an unknown field is a
/// datapack this build does not understand, not a field to skip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub struct NoiseGeneratorSettings {
    pub noise: NoiseSettings,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub noise_router: RouterFunctions,
    pub material_rule: ResourceLocation,
    pub spawn_target: Vec<SpawnTargetPoint>,
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    #[serde(default)]
    pub aquifers: Option<Aquifers>,
    pub legacy_random_source: bool,
    #[serde(default)]
    pub debug_functions: Vec<DebugFunction>,
}

/// A compiled density graph plus the eight root indices the generator reads.
///
/// A root that uses a kind the compiler cannot lower yet is replaced by a
/// constant zero and listed in [`NoiseRouter::failed_roots`], so the roots that
/// do compile stay usable.
pub struct NoiseRouter {
    program: Program,
    failed: Box<[(&'static str, CompileError)]>,
    material: Option<MaterialProgram>,
    sea_level: i32,
    noise_min_y: i32,
    noise_height: u32,
    world_seed: u64,
    default_block_state: VoxelId,
    default_fluid_state: VoxelId,
    beta: Option<Box<BetaTerrainNoises>>,
    cell_bounds: CellBounds,
}

impl NoiseRouter {
    pub(crate) fn new(
        program: Program,
        failed: Vec<(&'static str, CompileError)>,
        material: Option<MaterialProgram>,
        settings: &NoiseGeneratorSettings,
        world_seed: u64,
        default_block_state: VoxelId,
        default_fluid_state: VoxelId,
    ) -> Self {
        let cell_bounds = CellBounds::new(&program, program.root_node(FINAL_DENSITY));
        Self {
            program,
            failed: failed.into_boxed_slice(),
            material,
            sea_level: settings.sea_level,
            noise_min_y: settings.noise.min_y,
            noise_height: settings.noise.height,
            world_seed,
            default_block_state,
            default_fluid_state,
            beta: settings
                .legacy_random_source
                .then(|| Box::new(BetaTerrainNoises::new(world_seed))),
            cell_bounds,
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// `None` where the caller supplied no material rule registry, which is
    /// every path that only wants the density graph.
    pub fn material(&self) -> Option<&MaterialProgram> {
        self.material.as_ref()
    }

    pub fn temperature(&self) -> usize {
        TEMPERATURE
    }

    pub fn vegetation(&self) -> usize {
        VEGETATION
    }

    pub fn continents(&self) -> usize {
        CONTINENTS
    }

    pub fn erosion(&self) -> usize {
        EROSION
    }

    pub fn depth(&self) -> usize {
        DEPTH
    }

    pub fn ridges(&self) -> usize {
        RIDGES
    }

    pub fn chunk_surface_level(&self) -> usize {
        CHUNK_SURFACE_LEVEL
    }

    pub fn final_density(&self) -> usize {
        FINAL_DENSITY
    }

    /// The roots replaced by a constant zero, named as in [`ROOT_NAMES`].
    pub fn failed_roots(&self) -> &[(&'static str, CompileError)] {
        &self.failed
    }

    pub fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub fn noise_min_y(&self) -> i32 {
        self.noise_min_y
    }

    pub fn noise_height(&self) -> u32 {
        self.noise_height
    }

    pub fn world_seed(&self) -> u64 {
        self.world_seed
    }

    pub fn default_block_state(&self) -> VoxelId {
        self.default_block_state
    }

    pub fn default_fluid_state(&self) -> VoxelId {
        self.default_fluid_state
    }

    /// The Beta noises the surface rules read directly, outside the density
    /// graph. `None` on a modern generator.
    pub fn beta_noises(&self) -> Option<&BetaTerrainNoises> {
        self.beta.as_deref()
    }
}

impl NoiseRouter {
    /// Writes `volume.len()` values for `root` into `out`.
    pub fn fill(&self, ws: &mut Workspace, volume: &Volume, root: usize, out: &mut [f32]) {
        self.program.fill(ws, volume, root, out);
    }

    /// One `volume.len()`-long row per root, in the order given.
    pub fn fill_roots(
        &self,
        ws: &mut Workspace,
        volume: &Volume,
        roots: &[usize],
        out: &mut [f32],
    ) {
        let n = volume.len();
        assert_eq!(out.len(), roots.len() * n, "one row per root");
        for (k, &root) in roots.iter().enumerate() {
            self.program
                .fill(ws, volume, root, &mut out[k * n..(k + 1) * n]);
        }
    }

    /// [`NoiseRouter::fill_roots`] for nodes inside the graph rather than roots,
    /// which is what [`NoiseRouter::cell_inputs`] names.
    pub fn fill_nodes(
        &self,
        ws: &mut Workspace,
        volume: &Volume,
        nodes: &[NodeId],
        out: &mut [f32],
    ) {
        let n = volume.len();
        assert_eq!(out.len(), nodes.len() * n, "one row per node");
        for (k, &node) in nodes.iter().enumerate() {
            self.program
                .fill_node(ws, volume, node, &mut out[k * n..(k + 1) * n]);
        }
    }

    /// Hand `ws` the cell lattice already sampled over `volume`, laid out one row
    /// per [`NoiseRouter::cell_inputs`] entry, so the block fills that follow
    /// interpolate from it instead of resampling every input once per cell.
    pub fn pin_cell_lattice(&self, ws: &mut Workspace, volume: &Volume, values: &[f32]) {
        let stride = volume.len();
        assert_eq!(
            values.len(),
            self.cell_bounds.wrappers().len() * stride,
            "one row per interpolated wrapper"
        );
        for (k, &wrapper) in self.cell_bounds.wrappers().iter().enumerate() {
            let Node::Interpolated { cell, .. } = self.program.node(wrapper) else {
                unreachable!("the wrapper list holds only interpolated nodes")
            };
            ws.pin_lattice(*cell, volume, &values[k * stride..(k + 1) * stride]);
        }
    }

    /// The `interpolated` inputs to sample over the cell-corner lattice, in the
    /// order [`NoiseRouter::final_density_cell_bounds`] expects their corner
    /// intervals.
    #[inline]
    pub fn cell_inputs(&self) -> &[NodeId] {
        self.cell_bounds.inputs()
    }

    /// The cell lattice `final_density` interpolates on, or `None` when its
    /// wrappers disagree and there is no single one.
    #[inline]
    pub fn cell_size(&self) -> Option<IVec3> {
        self.cell_bounds.cell_size()
    }

    /// Bounds on `final_density` across a whole cell, given each `interpolated`
    /// wrapper's own bounds over the cell's eight corners.
    ///
    /// `None` when a term above the wrappers has a kind interval arithmetic
    /// cannot bound, which means the caller must fill the cell block by block.
    pub fn final_density_cell_bounds(&self, corners: &[Interval]) -> Option<Interval> {
        self.cell_bounds.eval(&self.program, corners)
    }

    /// Temperature and vegetation over the 16x16 block footprint of a chunk,
    /// indexed `x * 16 + z`, as the Beta density computation reads them.
    pub fn sample_beta_climate_grids(
        &self,
        ws: &mut Workspace,
        block_x: i32,
        block_z: i32,
    ) -> ([f32; 256], [f32; 256]) {
        let volume = Volume::new(
            IVec3::new(16, 1, 16),
            IVec3::new(block_x, 0, block_z),
            IVec3::ONE,
        );
        let mut values = vec![0.0f32; 2 * volume.len()];
        self.fill_roots(
            ws,
            &volume,
            &[self.temperature(), self.vegetation()],
            &mut values,
        );
        let mut temperature = [0.0f32; 256];
        let mut vegetation = [0.0f32; 256];
        for x in 0..16i32 {
            for z in 0..16i32 {
                let source = volume.index_unchecked(x, 0, z);
                temperature[(x * 16 + z) as usize] = values[source];
                vegetation[(x * 16 + z) as usize] = values[volume.len() + source];
            }
        }
        (temperature, vegetation)
    }

    /// Temperature and vegetation at one block column.
    pub fn sample_beta_climate(
        &self,
        ws: &mut Workspace,
        block_x: i32,
        block_z: i32,
    ) -> (f32, f32) {
        let volume = Volume::point(IVec3::new(block_x, 0, block_z));
        let mut values = [0.0f32; 2];
        self.fill_roots(
            ws,
            &volume,
            &[self.temperature(), self.vegetation()],
            &mut values,
        );
        (values[0], values[1])
    }
}
