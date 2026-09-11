use std::ops::Range;
use std::sync::Arc;
use std::sync::LazyLock;

use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
#[cfg(any(test, feature = "test-support"))]
use mcrs_voxel_storage::{Blocks, BoxVolume, Volume};
use mcrs_voxel_storage::{BlocksMut, VoxelId};

use super::placement::{HeightmapName, PlacementModifier, VerticalDirection};
use crate::noise::simplex::SimplexNoise;
use crate::value_provider::{HeightContext, VerticalAnchor};

/// The reference's `Biome.BIOME_INFO_NOISE`: seeded 2345, no world seed in it,
/// so every world's flower and dripstone counts follow the same field.
static BIOME_INFO_NOISE: LazyLock<SimplexNoise> =
    LazyLock::new(|| SimplexNoise::from_random_at_origin(&mut LegacyRandom::new(2345)));

/// The state sets every generator asks about and no feature configures: the
/// same for every feature of a dimension, resolved once at freeze.
#[derive(Clone, Debug, Default)]
pub struct WorldStates {
    pub air: VoxelId,
    pub cave_air: VoxelId,
    pub water: VoxelId,
    pub lava: VoxelId,
    /// `BlockState.isAir()`: air, cave air and void air.
    pub air_states: StateMask,
    /// Every state of the water block, source and flowing.
    pub water_states: StateMask,
    pub lava_states: StateMask,
    /// Every state whose fluid is water, waterlogged blocks included: `isWaterAt`.
    pub water_fluid: StateMask,
    /// Every state holding source water: `FluidState.isSource` over water.
    pub water_source: StateMask,
    pub lava_fluid: StateMask,
    pub any_source_fluid: StateMask,
    pub any_fluid: StateMask,
    pub replaceable: StateMask,
    /// `BlockBehaviour.isSolid`, the legacy flag.
    pub solid: StateMask,
    pub solid_render: StateMask,
    /// `isCollisionShapeFullBlock`, which is what `isFaceSturdy(UP)` reads on a
    /// full cube: the floor most features ask for.
    pub sturdy_up: StateMask,
    /// States whose collision shape is empty.
    pub empty_collision: StateMask,
    pub bedrock: StateMask,
    /// The block each state id belongs to; a state past the table is its own
    /// block.
    pub block_of_state: Arc<[u32]>,
    /// Per block index, its properties as the digits of its state ids.
    pub layouts: Arc<[BlockLayout]>,
}

/// One property of a block as a digit of its state ids: the values in declared
/// order, each `stride` ids apart.
#[derive(Clone, Debug)]
pub struct PropertyLayout {
    pub name: Arc<str>,
    pub values: Arc<[Arc<str>]>,
    pub stride: u16,
}

#[derive(Clone, Debug, Default)]
pub struct BlockLayout {
    pub base: u16,
    pub properties: Vec<PropertyLayout>,
}

impl BlockLayout {
    pub fn property(&self, name: &str) -> Option<&PropertyLayout> {
        self.properties.iter().find(|p| &*p.name == name)
    }

    pub fn value_index(&self, state: VoxelId, property: &PropertyLayout) -> u16 {
        (state.0 - self.base) / property.stride % property.values.len() as u16
    }

    pub fn with_index(&self, state: VoxelId, property: &PropertyLayout, index: u16) -> VoxelId {
        let shift =
            (index as i32 - self.value_index(state, property) as i32) * property.stride as i32;
        VoxelId((state.0 as i32 + shift) as u16)
    }

    /// `BlockState.trySetValue`: a property this block lacks, or a value
    /// outside it, leaves the state alone.
    fn try_set(&self, state: VoxelId, name: &str, value: &str) -> VoxelId {
        let Some(property) = self.property(name) else {
            return state;
        };
        match property.values.iter().position(|held| &**held == value) {
            Some(index) => self.with_index(state, property, index as u16),
            None => state,
        }
    }

    /// `BlockState.withPropertiesOf`: every property `source` shares by name
    /// and value text, copied over.
    pub fn with_properties_of(
        &self,
        state: VoxelId,
        from: &BlockLayout,
        source: VoxelId,
    ) -> VoxelId {
        from.properties.iter().fold(state, |state, property| {
            let value = &property.values[from.value_index(source, property) as usize];
            self.try_set(state, &property.name, value)
        })
    }
}

impl WorldStates {
    pub fn layout_of(&self, state: VoxelId) -> Option<&BlockLayout> {
        self.layouts.get(self.block_of(state) as usize)
    }

    /// `SpeleothemUtils.isEmptyOrWater`: air, or the water block itself.
    pub fn is_empty_or_water(&self, state: VoxelId) -> bool {
        self.air_states.contains(state.0 as usize) || self.water_states.contains(state.0 as usize)
    }

    pub fn is_empty_or_water_or_lava(&self, state: VoxelId) -> bool {
        self.is_empty_or_water(state) || self.lava_states.contains(state.0 as usize)
    }

    pub fn block_of(&self, state: VoxelId) -> u32 {
        self.block_of_state
            .get(state.0 as usize)
            .copied()
            .unwrap_or(state.0 as u32)
    }
}

/// What a placement modifier or a generator may ask of the volume it runs in,
/// beyond the blocks themselves.
///
/// World-absolute coordinates throughout.
pub trait WorldGenVolume: BlocksMut {
    fn world(&self) -> &WorldStates;

    fn is_air(&self, p: IVec3) -> bool {
        self.holds(&self.world().air_states, p)
    }

    fn holds(&self, mask: &FixedBitSet, p: IVec3) -> bool {
        mask.contains(self.get(p).0 as usize)
    }

    /// `Feature.safeSetBlock`: the write happens unless the block there is in
    /// `unless`, and the answer is whether it did.
    fn set_unless(&mut self, unless: &FixedBitSet, p: IVec3, state: VoxelId) -> bool {
        if self.holds(unless, p) {
            return false;
        }
        self.set(p, state);
        true
    }

    fn height(&self, kind: HeightmapName, x: i32, z: i32) -> i32;

    fn biome(&self, p: IVec3) -> u32;

    fn extent(&self) -> HeightContext;

    /// `BlockState.canSurvive`, which is block behaviour rather than a property
    /// of the state, so only the volume can answer it.
    fn would_survive(&self, state: VoxelId, p: IVec3) -> bool;
}

/// A set of block states as a bit per state id.
pub type StateMask = Arc<FixedBitSet>;

/// A set of biomes as a bit per the index [`WorldGenVolume::biome`] answers with.
pub type BiomeMask = Arc<FixedBitSet>;

/// A `BlockPredicate` with every set it names reduced to a mask.
#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    True,
    MatchingStates {
        offset: IVec3,
        states: StateMask,
    },
    MatchingBiomes(BiomeMask),
    WouldSurvive {
        offset: IVec3,
        state: VoxelId,
    },
    InsideWorldBounds {
        offset_y: i32,
    },
    HeightRange {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    VolumeMatch {
        min: [i32; 3],
        max: [i32; 3],
        matches: Box<Predicate>,
    },
    AnyOf(Vec<Predicate>),
    AllOf(Vec<Predicate>),
    Not(Box<Predicate>),
}

impl Predicate {
    pub fn test<W: WorldGenVolume>(&self, volume: &W, pos: IVec3) -> bool {
        match self {
            Predicate::True => true,
            Predicate::MatchingStates { offset, states } => {
                states.contains(volume.get(pos + *offset).0 as usize)
            }
            Predicate::MatchingBiomes(biomes) => biomes.contains(volume.biome(pos) as usize),
            Predicate::WouldSurvive { offset, state } => {
                volume.would_survive(*state, pos + *offset)
            }
            Predicate::InsideWorldBounds { offset_y } => volume.extent().contains(pos.y + offset_y),
            Predicate::HeightRange {
                min_inclusive,
                max_inclusive,
            } => {
                let extent = volume.extent();
                pos.y >= min_inclusive.resolve_y(extent) && pos.y <= max_inclusive.resolve_y(extent)
            }
            Predicate::VolumeMatch { min, max, matches } => {
                for x in min[0]..=max[0] {
                    for z in min[2]..=max[2] {
                        for y in min[1]..=max[1] {
                            if !matches.test(volume, IVec3::new(pos.x + x, pos.y + y, pos.z + z)) {
                                return false;
                            }
                        }
                    }
                }
                true
            }
            Predicate::AnyOf(predicates) => predicates.iter().any(|p| p.test(volume, pos)),
            Predicate::AllOf(predicates) => predicates.iter().all(|p| p.test(volume, pos)),
            Predicate::Not(predicate) => !predicate.test(volume, pos),
        }
    }
}

/// A `RuleTest` with every block, block state and tag it names reduced to a
/// mask.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    AlwaysTrue,
    MatchingStates(StateMask),
    HeightMatch {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    /// The membership test short-circuits the draw: a state outside the mask
    /// never reaches `nextFloat`.
    RandomStates {
        states: StateMask,
        probability: f32,
    },
    AllOf(Vec<Rule>),
    AnyOf(Vec<Rule>),
    Not(Box<Rule>),
}

pub fn mask_of(states: impl IntoIterator<Item = impl Into<u16>>) -> StateMask {
    Arc::new(
        states
            .into_iter()
            .map(|state| usize::from(state.into()))
            .collect(),
    )
}

pub fn single_state(state: VoxelId) -> StateMask {
    mask_of([state.0])
}

impl Rule {
    pub fn test<R: Random>(&self, state: VoxelId, y: i32, rng: &mut R) -> bool {
        match self {
            Rule::AlwaysTrue => true,
            Rule::MatchingStates(states) => states.contains(state.0 as usize),
            Rule::HeightMatch {
                min_inclusive,
                max_inclusive,
            } => *min_inclusive <= y && y <= *max_inclusive,
            Rule::RandomStates {
                states,
                probability,
            } => states.contains(state.0 as usize) && rng.next_f32() < *probability,
            Rule::AllOf(rules) => rules.iter().all(|rule| rule.test(state, y, rng)),
            Rule::AnyOf(rules) => rules.iter().any(|rule| rule.test(state, y, rng)),
            Rule::Not(rule) => !rule.test(state, y, rng),
        }
    }
}

/// A placement modifier with every set it names reduced to a mask.
pub type Modifier = PlacementModifier<Predicate>;

pub fn biome_info_noise(x: f64, z: f64) -> f64 {
    BIOME_INFO_NOISE.sample_2d(x, z, 1.0, 1.0) as f32 as f64
}

impl PlacementModifier<Predicate> {
    fn apply<W: WorldGenVolume, R: Random>(
        &self,
        volume: &W,
        rng: &mut R,
        origin: IVec3,
        carries: &dyn Fn(u32) -> bool,
        out: &mut Vec<IVec3>,
    ) {
        use PlacementModifier::*;
        match self {
            BlockPredicateFilter { predicate } => {
                if predicate.test(volume, origin) {
                    out.push(origin);
                }
            }
            RarityFilter { chance } => {
                if rng.next_f32() < 1.0 / chance.0 as f32 {
                    out.push(origin);
                }
            }
            RandomChance { chance } => {
                if rng.next_f32() < chance.0 as f32 {
                    out.push(origin);
                }
            }
            SurfaceRelativeThresholdFilter {
                heightmap,
                min_inclusive,
                max_inclusive,
            } => {
                let surface = volume.height(*heightmap, origin.x, origin.z) as i64;
                let y = origin.y as i64;
                if surface + min_inclusive.0 as i64 <= y && y <= surface + max_inclusive.0 as i64 {
                    out.push(origin);
                }
            }
            SurfaceWaterDepthFilter { max_water_depth } => {
                let floor = volume.height(HeightmapName::OceanFloor, origin.x, origin.z);
                let surface = volume.height(HeightmapName::WorldSurface, origin.x, origin.z);
                if surface - floor <= *max_water_depth {
                    out.push(origin);
                }
            }
            Biome {} => {
                if carries(volume.biome(origin)) {
                    out.push(origin);
                }
            }
            Count { count } => {
                out.extend(std::iter::repeat_n(
                    origin,
                    count.sample(rng).max(0) as usize,
                ));
            }
            NoiseBasedCount {
                noise_to_count_ratio,
                noise_factor,
                noise_offset,
            } => {
                let noise = biome_info_noise(
                    origin.x as f64 / *noise_factor,
                    origin.z as f64 / *noise_factor,
                );
                let count = ((noise + *noise_offset) * *noise_to_count_ratio as f64).ceil() as i32;
                out.extend(std::iter::repeat_n(origin, count.max(0) as usize));
            }
            NoiseThresholdCount {
                noise_level,
                below_noise,
                above_noise,
            } => {
                let noise = biome_info_noise(origin.x as f64 / 200.0, origin.z as f64 / 200.0);
                let count = if noise < *noise_level {
                    *below_noise
                } else {
                    *above_noise
                };
                out.extend(std::iter::repeat_n(origin, count.max(0) as usize));
            }
            CountOnEveryLayer { count } => {
                let mut layer = 0;
                loop {
                    let mut found_any = false;
                    let mut i = 0;
                    while i < count.sample(rng) {
                        let x = rng.next_i32_bound(16) + origin.x;
                        let z = rng.next_i32_bound(16) + origin.z;
                        let start = volume.height(HeightmapName::MotionBlocking, x, z);
                        if let Some(y) = on_ground_y(volume, x, start, z, layer) {
                            out.push(IVec3::new(x, y, z));
                            found_any = true;
                        }
                        i += 1;
                    }
                    layer += 1;
                    if !found_any {
                        break;
                    }
                }
            }
            Cuboid {
                xz_size,
                y_size,
                include_edges,
                include_interior,
            } => {
                let height = y_size.sample(rng);
                let width = xz_size.sample(rng);
                let length = xz_size.sample(rng);
                for x in 0..=width {
                    for y in 0..=height {
                        for z in 0..=length {
                            let edge_xy =
                                *include_edges || (x != 0 && x != width) || (y != 0 && y != height);
                            let edge_zy = *include_edges
                                || (z != 0 && z != length)
                                || (y != 0 && y != height);
                            let edge_xz =
                                *include_edges || (x != 0 && x != width) || (z != 0 && z != length);
                            let interior = *include_interior
                                || x == 0
                                || x == width
                                || y == 0
                                || y == height
                                || z == 0
                                || z == length;
                            if edge_xy && edge_zy && edge_xz && interior {
                                out.push(IVec3::new(x + origin.x, y + origin.y, z + origin.z));
                            }
                        }
                    }
                }
            }
            EnvironmentScan {
                direction_of_search,
                target_condition,
                allowed_search_condition,
                max_steps,
            } => {
                let allowed = |pos| {
                    allowed_search_condition
                        .as_ref()
                        .is_none_or(|p| p.test(volume, pos))
                };
                if !allowed(origin) {
                    return;
                }
                let step = match direction_of_search {
                    VerticalDirection::Up => 1,
                    VerticalDirection::Down => -1,
                };
                let extent = volume.extent();
                let mut pos = origin;
                for _ in 0..max_steps.0 {
                    if target_condition.test(volume, pos) {
                        out.push(pos);
                        return;
                    }
                    pos.y += step;
                    if !extent.contains(pos.y) {
                        return;
                    }
                    if !allowed(pos) {
                        break;
                    }
                }
                if target_condition.test(volume, pos) {
                    out.push(pos);
                }
            }
            Heightmap { heightmap } => {
                let height = volume.height(*heightmap, origin.x, origin.z);
                if height > volume.extent().min_y {
                    out.push(IVec3::new(origin.x, height, origin.z));
                }
            }
            HeightRange { height } => {
                let y = height.sample(rng, volume.extent());
                out.push(IVec3::new(origin.x, y, origin.z));
            }
            InSquare {} => {
                let x = rng.next_i32_bound(16) + origin.x;
                let z = rng.next_i32_bound(16) + origin.z;
                out.push(IVec3::new(x, origin.y, z));
            }
            Offset { x, y, z } => {
                let dx = x.sample(rng);
                let dy = y.sample(rng);
                let dz = z.sample(rng);
                out.push(IVec3::new(origin.x + dx, origin.y + dy, origin.z + dz));
            }
            RandomlySelected { placements } => {
                let chosen = rng.next_i32_bound(placements.len() as i32) as usize;
                placements[chosen].apply(volume, rng, origin, carries, out);
            }
            FixedPlacement { positions } => {
                let chunk_x = origin.x >> 4;
                let chunk_z = origin.z >> 4;
                for &position in positions {
                    let position = IVec3::from_array(position);
                    if position.x >> 4 == chunk_x && position.z >> 4 == chunk_z {
                        out.push(position);
                    }
                }
            }
        }
    }
}

/// `CountOnEveryLayerPlacement.findOnGroundYPosition`: the first solid top face
/// below the start that is the `layer`-th one down. Empty is air, water or lava.
fn on_ground_y<W: WorldGenVolume>(
    volume: &W,
    x: i32,
    y_start: i32,
    z: i32,
    layer_to_place_on: i32,
) -> Option<i32> {
    let world = volume.world();
    let is_empty = |state: VoxelId| world.is_empty_or_water_or_lava(state);
    let bedrock = &world.bedrock;
    let min_y = volume.extent().min_y;
    let mut current_layer = 0;
    let mut current = volume.get(IVec3::new(x, y_start, z));
    let mut y = y_start;
    while y >= min_y + 1 {
        let below = volume.get(IVec3::new(x, y - 1, z));
        if !is_empty(below) && is_empty(current) && !bedrock.contains(below.0 as usize) {
            if current_layer == layer_to_place_on {
                return Some(y);
            }
            current_layer += 1;
        }
        current = below;
        y -= 1;
    }
    None
}

/// The stack of positions with the modifier each is waiting on, plus the
/// output buffer, one set per worker. Cleared between objects, never
/// reallocated once warm.
#[derive(Debug, Default)]
pub struct PlacerScratch {
    pending: Vec<(IVec3, usize)>,
    outputs: Vec<IVec3>,
}

/// One object: run its modifier chain over `origin` and hand every surviving
/// position to `generate`.
///
/// The stack machine is the reference's, reverse push included — the random
/// source is shared by every modifier and by the generator, so the traversal
/// order is what fixes the draw order.
pub fn place<W: WorldGenVolume, R: Random>(
    modifiers: &[Modifier],
    volume: &mut W,
    scratch: &mut PlacerScratch,
    origin: IVec3,
    rng: &mut R,
    carries: &dyn Fn(u32) -> bool,
    generate: &mut dyn FnMut(&mut W, &mut R, IVec3) -> bool,
) -> bool {
    if modifiers.is_empty() {
        return generate(volume, rng, origin);
    }

    scratch.pending.clear();
    scratch.pending.push((origin, 0));

    let mut placed_any = false;
    while let Some((pos, index)) = scratch.pending.pop() {
        modifiers[index].apply(volume, rng, pos, carries, &mut scratch.outputs);
        let next = index + 1;
        if next < modifiers.len() {
            scratch
                .pending
                .extend(scratch.outputs.iter().rev().map(|&at| (at, next)));
        } else {
            for &at in &scratch.outputs {
                placed_any |= generate(volume, rng, at);
            }
        }
        scratch.outputs.clear();
    }
    placed_any
}

/// One object's generator: the object's `(step, index)`, the volume, the shared
/// source, the position, and whether a biome carries the object being placed —
/// which a feature holding a nested placed feature passes down unchanged.
pub type Generate<'a, W> = &'a mut dyn FnMut(
    (usize, usize),
    &mut W,
    &mut XoroshiroRandom,
    IVec3,
    &dyn Fn(u32) -> bool,
) -> bool;

/// `steps` of a volume's decoration program: each step in order, every feature
/// of a step that the volume's biomes carry, ascending by global index, each on
/// its own source seeded from the column's decoration seed.
///
/// A feature's seed is a function of its own `(step, index)` alone, so a caller
/// that walks the program a few steps at a time draws exactly what one call
/// over the whole range would.
///
/// `present[step]` is the union over the volume's biomes; `carries` answers
/// whether one biome carries the feature at `(step, index)`, which is what the
/// `biome` filter tests.
#[allow(clippy::too_many_arguments)]
pub fn decorate<'a, W: WorldGenVolume>(
    present: &[FixedBitSet],
    steps: Range<usize>,
    chain: &dyn Fn(usize, usize) -> &'a [Modifier],
    carries: &dyn Fn(u32, usize, usize) -> bool,
    volume: &mut W,
    scratch: &mut PlacerScratch,
    origin: IVec3,
    decoration_seed: i64,
    generate: Generate<'_, W>,
) {
    for step in steps {
        let Some(bits) = present.get(step) else {
            continue;
        };
        for index in bits.ones() {
            let seed = decoration_seed
                .wrapping_add(index as i64)
                .wrapping_add(10_000 * step as i64);
            let mut rng = XoroshiroRandom::new(seed as u64);
            let carry = |biome: u32| carries(biome, step, index);
            place(
                chain(step, index),
                volume,
                scratch,
                origin,
                &mut rng,
                &carry,
                &mut |volume, rng, at| generate((step, index), volume, rng, at, &carry),
            );
        }
    }
}

/// A volume over one owned box: one biome, a fixed extent, a height answered by
/// a closure over the box, and every write logged in order. What a test world
/// needs, and nothing a real column has.
#[cfg(any(test, feature = "test-support"))]
pub struct BoxRegion {
    pub blocks: BoxVolume,
    pub world: WorldStates,
    pub extent: HeightContext,
    pub biome: u32,
    pub height: Box<dyn Fn(&BoxVolume, HeightmapName, i32, i32) -> i32>,
    pub writes: Vec<(IVec3, VoxelId)>,
}

#[cfg(any(test, feature = "test-support"))]
impl BoxRegion {
    /// Every cell of `min..=max` is `fill`; the extent is the box's own height
    /// and the height map answers one past its top.
    pub fn new(min: IVec3, max: IVec3, fill: VoxelId) -> Self {
        BoxRegion {
            blocks: BoxVolume::filled(min, max, fill),
            world: WorldStates::default(),
            extent: HeightContext {
                min_y: min.y,
                depth: max.y - min.y + 1,
                sea_level: 63,
            },
            biome: 0,
            height: Box::new(move |_, _, _, _| max.y + 1),
            writes: Vec::new(),
        }
    }

    /// `radius` columns of sixteen around the origin, from `min_y` to `max_y`.
    pub fn columns(radius: i32, min_y: i32, max_y: i32, fill: VoxelId) -> Self {
        Self::new(
            IVec3::new(-radius * 16, min_y, -radius * 16),
            IVec3::new(radius * 16 + 15, max_y, radius * 16 + 15),
            fill,
        )
    }

    /// Every cell from the bottom of the box up to and including `top`.
    pub fn floor(mut self, top: i32, id: VoxelId) -> Self {
        for y in self.blocks.min().y..=top {
            self.blocks.fill_layer(y, id);
        }
        self
    }

    pub fn with_height(
        mut self,
        height: impl Fn(&BoxVolume, HeightmapName, i32, i32) -> i32 + 'static,
    ) -> Self {
        self.height = Box::new(height);
        self
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Volume for BoxRegion {
    fn min(&self) -> IVec3 {
        self.blocks.min()
    }

    fn max(&self) -> IVec3 {
        self.blocks.max()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Blocks for BoxRegion {
    fn get(&self, p: IVec3) -> VoxelId {
        self.blocks.get(p)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl BlocksMut for BoxRegion {
    fn set(&mut self, p: IVec3, id: VoxelId) {
        if self.blocks.contains(p) {
            self.blocks.set(p, id);
            self.writes.push((p, id));
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl WorldGenVolume for BoxRegion {
    fn world(&self) -> &WorldStates {
        &self.world
    }

    fn height(&self, kind: HeightmapName, x: i32, z: i32) -> i32 {
        (self.height)(&self.blocks, kind, x, z)
    }

    fn biome(&self, _: IVec3) -> u32 {
        self.biome
    }

    fn extent(&self) -> HeightContext {
        self.extent
    }

    fn would_survive(&self, _: VoxelId, _: IVec3) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::tree::{Bounded, UnitFloat};
    use crate::value_provider::{BoundedIntProvider, IntProvider};

    const AIR: VoxelId = VoxelId(0);
    const STONE: VoxelId = VoxelId(1);

    /// Stone to y=64, air above, one heightmap value, one biome.
    fn stub() -> BoxRegion {
        let mut volume = BoxRegion::columns(4, -64, 319, AIR)
            .floor(64, STONE)
            .with_height(|_, _, _, _| 65);
        volume.biome = 7;
        volume.world.air_states = mask_of([AIR.0]);
        volume
    }

    fn always(_: u32) -> bool {
        true
    }

    /// Runs a chain from `origin` and reports where the generator fired and what
    /// the shared source looked like afterwards.
    fn run(modifiers: &[Modifier], origin: IVec3, seed: u64) -> (Vec<IVec3>, XoroshiroRandom) {
        let mut volume = stub();
        let mut scratch = PlacerScratch::default();
        let mut rng = XoroshiroRandom::new(seed);
        let mut hits = Vec::new();
        place(
            modifiers,
            &mut volume,
            &mut scratch,
            origin,
            &mut rng,
            &always,
            &mut |_, _, at| {
                hits.push(at);
                true
            },
        );
        (hits, rng)
    }

    const ORIGIN: IVec3 = IVec3::new(16, 0, 32);

    #[test]
    fn an_empty_chain_runs_the_generator_at_the_origin() {
        let (hits, rng) = run(&[], ORIGIN, 1);
        assert_eq!(hits, vec![ORIGIN]);
        assert_eq!(rng, XoroshiroRandom::new(1), "an empty chain draws nothing");
    }

    /// `count` emits three copies and `offset` consumes them one at a time. A
    /// placer that pushed its outputs in emission order would run the three
    /// offsets in the reverse order and place a different world, so pin both the
    /// draw sequence and where each copy landed.
    #[test]
    fn outputs_are_consumed_depth_first_in_emission_order() {
        let modifiers = vec![
            Modifier::Count {
                count: BoundedIntProvider(IntProvider::Constant(3)),
            },
            Modifier::Offset {
                x: BoundedIntProvider(IntProvider::uniform(0, 15)),
                y: BoundedIntProvider(IntProvider::Constant(0)),
                z: BoundedIntProvider(IntProvider::Constant(0)),
            },
        ];
        let (hits, rng) = run(&modifiers, ORIGIN, 42);

        let mut replay = XoroshiroRandom::new(42);
        let expected: Vec<IVec3> = (0..3)
            .map(|_| {
                IVec3::new(
                    ORIGIN.x + IntProvider::uniform(0, 15).sample(&mut replay),
                    0,
                    ORIGIN.z,
                )
            })
            .collect();
        assert_eq!(rng, replay, "three offset draws and nothing else");
        assert_eq!(hits, expected);
    }

    /// The whole first branch reaches the generator before the second branch's
    /// filter is rolled at all.
    #[test]
    fn a_filter_after_a_count_interleaves_one_branch_at_a_time() {
        let modifiers = vec![
            Modifier::Count {
                count: BoundedIntProvider(IntProvider::Constant(2)),
            },
            Modifier::RandomChance {
                chance: UnitFloat(0.5),
            },
            Modifier::Offset {
                x: BoundedIntProvider(IntProvider::uniform(0, 7)),
                y: BoundedIntProvider(IntProvider::Constant(0)),
                z: BoundedIntProvider(IntProvider::Constant(0)),
            },
        ];
        let (hits, rng) = run(&modifiers, ORIGIN, 9);

        let mut replay = XoroshiroRandom::new(9);
        let mut expected = Vec::new();
        for _ in 0..2 {
            if replay.next_f32() < 0.5 {
                let x = IntProvider::uniform(0, 7).sample(&mut replay);
                expected.push(IVec3::new(ORIGIN.x + x, 0, ORIGIN.z));
            }
        }
        assert_eq!(rng, replay);
        assert_eq!(hits, expected);
    }

    #[test]
    fn in_square_draws_two_bounded_ints() {
        let (hits, rng) = run(&[Modifier::InSquare {}], ORIGIN, 7);
        let mut replay = XoroshiroRandom::new(7);
        let x = replay.next_i32_bound(16) + ORIGIN.x;
        let z = replay.next_i32_bound(16) + ORIGIN.z;
        assert_eq!(rng, replay);
        assert_eq!(hits, vec![IVec3::new(x, 0, z)]);
    }

    #[test]
    fn heightmap_moves_to_the_map_and_draws_nothing() {
        let (hits, rng) = run(
            &[Modifier::Heightmap {
                heightmap: HeightmapName::WorldSurfaceWg,
            }],
            ORIGIN,
            3,
        );
        assert_eq!(rng, XoroshiroRandom::new(3));
        assert_eq!(hits, vec![IVec3::new(16, 65, 32)]);
    }

    #[test]
    fn a_block_predicate_filter_draws_nothing() {
        let modifiers = vec![
            Modifier::Heightmap {
                heightmap: HeightmapName::WorldSurfaceWg,
            },
            Modifier::BlockPredicateFilter {
                predicate: Predicate::MatchingStates {
                    offset: IVec3::NEG_Y,
                    states: mask_of([STONE.0]),
                },
            },
        ];
        let (hits, rng) = run(&modifiers, ORIGIN, 5);
        assert_eq!(rng, XoroshiroRandom::new(5));
        assert_eq!(hits, vec![IVec3::new(16, 65, 32)]);
    }

    #[test]
    fn cuboid_samples_the_height_before_the_two_widths() {
        let modifiers = vec![Modifier::Cuboid {
            xz_size: BoundedIntProvider(IntProvider::uniform(1, 3)),
            y_size: BoundedIntProvider(IntProvider::uniform(1, 3)),
            include_edges: true,
            include_interior: true,
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 11);

        let mut replay = XoroshiroRandom::new(11);
        let height = IntProvider::uniform(1, 3).sample(&mut replay);
        let width = IntProvider::uniform(1, 3).sample(&mut replay);
        let length = IntProvider::uniform(1, 3).sample(&mut replay);
        assert_eq!(rng, replay);
        assert_eq!(hits.len() as i32, (width + 1) * (height + 1) * (length + 1));
        assert_eq!(hits[0], ORIGIN);
        assert_eq!(
            hits[1],
            ORIGIN + IVec3::new(0, 0, 1),
            "z is the innermost axis"
        );
    }

    #[test]
    fn a_cuboid_shell_drops_the_interior() {
        let modifiers = vec![Modifier::Cuboid {
            xz_size: BoundedIntProvider(IntProvider::Constant(2)),
            y_size: BoundedIntProvider(IntProvider::Constant(2)),
            include_edges: true,
            include_interior: false,
        }];
        let (hits, _) = run(&modifiers, ORIGIN, 1);
        assert_eq!(hits.len(), 26, "a 3x3x3 without its centre");
    }

    #[test]
    fn randomly_selected_spends_one_draw_then_its_choice() {
        let modifiers = vec![Modifier::RandomlySelected {
            placements: vec![
                Modifier::InSquare {},
                Modifier::Offset {
                    x: BoundedIntProvider(IntProvider::Constant(1)),
                    y: BoundedIntProvider(IntProvider::Constant(2)),
                    z: BoundedIntProvider(IntProvider::Constant(3)),
                },
            ],
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 21);

        let mut replay = XoroshiroRandom::new(21);
        let expected = match replay.next_i32_bound(2) {
            0 => {
                let x = replay.next_i32_bound(16) + ORIGIN.x;
                let z = replay.next_i32_bound(16) + ORIGIN.z;
                IVec3::new(x, 0, z)
            }
            _ => ORIGIN + IVec3::new(1, 2, 3),
        };
        assert_eq!(rng, replay);
        assert_eq!(hits, vec![expected]);
    }

    #[test]
    fn rarity_and_random_chance_each_spend_one_float() {
        for modifier in [
            Modifier::RarityFilter { chance: Bounded(4) },
            Modifier::RandomChance {
                chance: UnitFloat(0.25),
            },
        ] {
            let (_, rng) = run(&[modifier], ORIGIN, 1);
            let mut replay = XoroshiroRandom::new(1);
            replay.next_f32();
            assert_eq!(rng, replay);
        }
    }

    #[test]
    fn environment_scan_walks_to_the_target_and_draws_nothing() {
        let modifiers = vec![Modifier::EnvironmentScan {
            direction_of_search: VerticalDirection::Down,
            target_condition: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([STONE.0]),
            },
            allowed_search_condition: None,
            max_steps: Bounded(32),
        }];
        let (hits, rng) = run(&modifiers, IVec3::new(0, 70, 0), 4);
        assert_eq!(rng, XoroshiroRandom::new(4));
        assert_eq!(hits, vec![IVec3::new(0, 64, 0)]);
    }

    #[test]
    fn environment_scan_gives_up_once_the_search_condition_fails() {
        let modifiers = vec![Modifier::EnvironmentScan {
            direction_of_search: VerticalDirection::Down,
            target_condition: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: StateMask::default(),
            },
            allowed_search_condition: Some(Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([AIR.0]),
            }),
            max_steps: Bounded(32),
        }];
        let (hits, _) = run(&modifiers, IVec3::new(0, 70, 0), 4);
        assert!(hits.is_empty());
    }

    #[test]
    fn the_biome_filter_asks_the_window_and_draws_nothing() {
        let mut volume = stub();
        let mut scratch = PlacerScratch::default();
        let mut rng = XoroshiroRandom::new(0);
        let mut hits = 0;
        let refuse = |biome: u32| biome != 7;
        place(
            &[Modifier::Biome {}],
            &mut volume,
            &mut scratch,
            IVec3::ZERO,
            &mut rng,
            &refuse,
            &mut |_, _, _| {
                hits += 1;
                true
            },
        );
        assert_eq!(hits, 0);
        assert_eq!(rng, XoroshiroRandom::new(0));
    }

    #[test]
    fn fixed_placement_keeps_only_its_own_chunk() {
        let modifiers = vec![Modifier::FixedPlacement {
            positions: vec![[20, 5, 35], [-3, 5, 35]],
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 6);
        assert_eq!(rng, XoroshiroRandom::new(6));
        assert_eq!(hits, vec![IVec3::new(20, 5, 35)]);
    }

    #[test]
    fn surface_water_depth_reads_two_maps_and_draws_nothing() {
        let (hits, rng) = run(
            &[Modifier::SurfaceWaterDepthFilter { max_water_depth: 0 }],
            ORIGIN,
            8,
        );
        assert_eq!(rng, XoroshiroRandom::new(8));
        assert_eq!(hits, vec![ORIGIN], "the stub's two maps are equal");
    }

    /// The count provider is sampled once per loop test, so the pass that finds
    /// nothing still spends the sample that ends it.
    #[test]
    fn count_on_every_layer_descends_and_stops_when_a_pass_finds_nothing() {
        let modifiers = vec![Modifier::CountOnEveryLayer {
            count: BoundedIntProvider(IntProvider::Constant(1)),
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 12);

        let mut replay = XoroshiroRandom::new(12);
        let x = replay.next_i32_bound(16) + ORIGIN.x;
        let z = replay.next_i32_bound(16) + ORIGIN.z;
        replay.next_i32_bound(16);
        replay.next_i32_bound(16);
        assert_eq!(
            rng, replay,
            "one hit on layer 0, then a second pass that finds none"
        );
        assert_eq!(hits, vec![IVec3::new(x, 65, z)]);
    }

    #[test]
    fn a_rule_test_draws_only_once_the_block_matched() {
        let rule = Rule::RandomStates {
            states: mask_of([STONE.0]),
            probability: 1.0,
        };
        let mut rng = XoroshiroRandom::new(2);
        assert!(!rule.test(AIR, 0, &mut rng));
        assert_eq!(rng, XoroshiroRandom::new(2), "no draw on a failed identity");
        assert!(rule.test(STONE, 0, &mut rng));
        let mut replay = XoroshiroRandom::new(2);
        replay.next_f32();
        assert_eq!(rng, replay);
    }

    #[test]
    fn the_scratch_is_drained_and_reused() {
        let modifiers = vec![
            Modifier::Count {
                count: BoundedIntProvider(IntProvider::Constant(4)),
            },
            Modifier::InSquare {},
        ];
        let mut volume = stub();
        let mut scratch = PlacerScratch::default();
        let mut rng = XoroshiroRandom::new(13);
        for _ in 0..8 {
            place(
                &modifiers,
                &mut volume,
                &mut scratch,
                IVec3::ZERO,
                &mut rng,
                &always,
                &mut |_, _, _| true,
            );
        }
        assert!(scratch.pending.is_empty());
        assert!(scratch.outputs.is_empty());
        assert!(scratch.pending.capacity() <= 8, "no unbounded growth");
    }

    /// Seed 2345 with no world seed in it, so this field is the same in every
    /// world. The two off-origin values are the reference's own noise, recorded
    /// so a change to the seed, the permutation stream or the sampler is caught;
    /// the origin is zero for any permutation table and pins the discarded
    /// lattice offset instead.
    #[test]
    fn the_count_noise_is_the_fixed_field() {
        assert_eq!(biome_info_noise(0.0, 0.0), 0.0);
        assert_eq!(
            biome_info_noise(100.0 / 200.0, 300.0 / 200.0),
            0.435_083_717_107_772_8
        );
        assert_eq!(biome_info_noise(-1.2, 3.7), 0.302_165_716_886_520_4);
    }

    #[test]
    fn the_step_loop_seeds_each_object_from_the_decoration_seed() {
        let in_square = || vec![Modifier::InSquare {}];
        let steps = vec![vec![in_square(), in_square()], vec![in_square()]];
        let mut present = vec![FixedBitSet::with_capacity(2), FixedBitSet::with_capacity(1)];
        present[0].insert(1);
        present[1].insert(0);

        let walk = |ranges: &[Range<usize>]| {
            let mut volume = stub();
            let mut scratch = PlacerScratch::default();
            let mut hits = Vec::new();
            for range in ranges {
                decorate(
                    &present,
                    range.clone(),
                    &|step, index| &steps[step][index],
                    &|_, _, _| true,
                    &mut volume,
                    &mut scratch,
                    ORIGIN,
                    777,
                    &mut |_, _, _, at, _| {
                        hits.push(at);
                        true
                    },
                );
            }
            hits
        };

        let expect = |seed: i64| {
            let mut rng = XoroshiroRandom::new(seed as u64);
            let x = rng.next_i32_bound(16) + ORIGIN.x;
            let z = rng.next_i32_bound(16) + ORIGIN.z;
            IVec3::new(x, 0, z)
        };
        let whole = walk(std::slice::from_ref(&(0..2)));
        assert_eq!(
            whole,
            vec![expect(777 + 1), expect(777 + 10_000)],
            "seed is decoration_seed + index + 10000 * step, and a skipped index shifts nothing"
        );
        assert_eq!(
            walk(&[0..1, 1..2]),
            whole,
            "a seed names its own step, so cutting the walk into rungs draws the same objects"
        );
    }
}
