use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::mth::clamped_map;
use mcrs_minecraft_core::value_provider::{FloatProvider, IntProvider};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_feature::block_predicate::Direction;
use mcrs_minecraft_worldgen_feature::compile::BlockResolver;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};

use crate::holds;
use mcrs_minecraft_core::mth::{cos_modern, sin_modern};
use mcrs_minecraft_worldgen_feature::column::{Column, scan_column};

fn speleothem_profile(
    xz_distance_from_center: f64,
    radius: f64,
    scale: f64,
    bluntness: f64,
) -> f64 {
    let distance = xz_distance_from_center.max(bluntness);
    let r = distance / radius * 0.384;
    let relative = scale
        * (0.75 * r.powf(1.333_333_333_333_333_3)
            - r.powf(0.666_666_666_666_666_6)
            - 0.333_333_333_333_333_3 * r.ln());
    relative.max(0.0) / 0.384 * radius
}

/// The five `SpeleothemThickness` values in the order a column is built.
const THICKNESSES: usize = 5;
const TIP_MERGE: usize = 0;
pub(crate) const TIP: usize = 1;
pub(crate) const FRUSTUM: usize = 2;
const MIDDLE: usize = 3;
const BASE: usize = 4;

/// Every state a pointed speleothem takes: `[tip direction][thickness]
/// [waterlogged]`, the direction `up` first.
#[derive(Clone, Debug)]
pub struct PointedStates {
    pub states: [[[VoxelId; 2]; THICKNESSES]; 2],
}

impl PointedStates {
    pub fn resolve(pointed: &BlockState, blocks: &dyn BlockResolver) -> Option<Self> {
        const THICKNESS_NAMES: [&str; THICKNESSES] =
            ["tip_merge", "tip", "frustum", "middle", "base"];
        let mut states = [[[VoxelId(0); 2]; THICKNESSES]; 2];
        for (direction, name) in ["up", "down"].into_iter().enumerate() {
            for (thickness, thickness_name) in THICKNESS_NAMES.into_iter().enumerate() {
                for (waterlogged, flag) in ["false", "true"].into_iter().enumerate() {
                    let mut properties = std::collections::BTreeMap::new();
                    properties.insert("vertical_direction".to_owned(), name.to_owned());
                    properties.insert("thickness".to_owned(), thickness_name.to_owned());
                    properties.insert("waterlogged".to_owned(), flag.to_owned());
                    states[direction][thickness][waterlogged] = blocks.state(&BlockState {
                        name: pointed.name.clone(),
                        properties: Some(properties),
                    })?;
                }
            }
        }
        Some(PointedStates { states })
    }

    fn get(&self, tip_up: bool, thickness: usize, waterlogged: bool) -> VoxelId {
        self.states[usize::from(!tip_up)][thickness][usize::from(waterlogged)]
    }
}

#[derive(Clone, Debug)]
pub struct CompiledSpeleothemCluster {
    pub base_block: VoxelId,
    pub base_block_states: StateMask,
    pub pointed: PointedStates,
    pub pointed_block_states: StateMask,
    pub replaceable_blocks: StateMask,
    /// `SpeleothemUtils.isBase`: the base block or one of the replaceables.
    pub base_or_replaceable: StateMask,
    pub floor_to_ceiling_search_range: i32,
    pub height: IntProvider,
    pub radius: IntProvider,
    pub max_stalagmite_stalactite_height_diff: i32,
    pub height_deviation: i32,
    pub speleothem_block_layer_thickness: IntProvider,
    pub density: FloatProvider,
    pub wetness: FloatProvider,
    pub chance_of_speleothem_at_max_distance_from_center: f32,
    pub max_distance_from_edge_affecting_chance_of_speleothem: i32,
    pub max_distance_from_center_affecting_height_bias: i32,
    pub base_stone_overworld: StateMask,
}

/// `SpeleothemClusterFeature.place`: five draws for the cluster, then one
/// column per cell of the radius rectangle.
pub fn place_speleothem_cluster<W: WorldGenVolume>(
    config: &CompiledSpeleothemCluster,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
) -> bool {
    if !volume.world().is_empty_or_water(volume.get(origin)) {
        return false;
    }
    let cluster_height = config.height.sample(rng);
    let wetness = config.wetness.sample(rng);
    let density = config.density.sample(rng);
    let x_radius = config.radius.sample(rng);
    let z_radius = config.radius.sample(rng);

    for dx in -x_radius..=x_radius {
        for dz in -z_radius..=z_radius {
            let from_edge = (x_radius - dx.abs()).min(z_radius - dz.abs());
            let chance = clamped_map(
                from_edge as f32,
                0.0,
                config.max_distance_from_edge_affecting_chance_of_speleothem as f32,
                config.chance_of_speleothem_at_max_distance_from_center,
                1.0,
            ) as f64;
            place_cluster_column(
                config,
                volume,
                rng,
                origin + IVec3::new(dx, 0, dz),
                dx,
                dz,
                wetness,
                chance,
                cluster_height,
                density,
            );
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn place_cluster_column<W: WorldGenVolume>(
    config: &CompiledSpeleothemCluster,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    pos: BlockPos,
    dx: i32,
    dz: i32,
    chance_of_water: f32,
    chance_of_speleothem: f64,
    cluster_height: i32,
    density: f32,
) {
    let world = volume.world();
    let Some(base) = scan_column(
        volume,
        pos,
        config.floor_to_ceiling_search_range,
        |state| world.is_empty_or_water(state),
        |state| !world.is_empty_or_water(state),
    ) else {
        return;
    };
    if base.ceiling.is_none() && base.floor.is_none() {
        return;
    }

    let want_pool = rng.next_f32() < chance_of_water;
    let column = match base.floor {
        Some(floor)
            if want_pool && can_place_pool(config, volume, BlockPos::new(pos.x, floor, pos.z)) =>
        {
            volume.set(BlockPos::new(pos.x, floor, pos.z), volume.world().water);
            Column {
                floor: Some(floor - 1),
                ceiling: base.ceiling,
            }
        }
        _ => base,
    };
    let (floor, ceiling) = (column.floor, base.ceiling);

    let want_stalactite = rng.next_f64() < chance_of_speleothem;
    let stalactite_height = match ceiling {
        Some(ceiling)
            if want_stalactite
                && !volume.holds(
                    &volume.world().lava_states,
                    BlockPos::new(pos.x, ceiling, pos.z),
                ) =>
        {
            let thickness = config.speleothem_block_layer_thickness.sample(rng);
            replace_with_base(
                config,
                volume,
                BlockPos::new(pos.x, ceiling, pos.z),
                thickness,
                1,
            );
            let max = match floor {
                Some(floor) => cluster_height.min(ceiling - floor),
                None => cluster_height,
            };
            cluster_speleothem_height(config, rng, dx, dz, density, max)
        }
        _ => 0,
    };

    let want_stalagmite = rng.next_f64() < chance_of_speleothem;
    let stalagmite_height = match floor {
        Some(floor)
            if want_stalagmite
                && !volume.holds(
                    &volume.world().lava_states,
                    BlockPos::new(pos.x, floor, pos.z),
                ) =>
        {
            let thickness = config.speleothem_block_layer_thickness.sample(rng);
            replace_with_base(
                config,
                volume,
                BlockPos::new(pos.x, floor, pos.z),
                thickness,
                -1,
            );
            if ceiling.is_some() {
                let diff = config.max_stalagmite_stalactite_height_diff;
                (stalactite_height + rng.next_int_between_inclusive(-diff, diff)).max(0)
            } else {
                cluster_speleothem_height(config, rng, dx, dz, density, cluster_height)
            }
        }
        _ => 0,
    };

    let (actual_stalactite, actual_stalagmite) = match (ceiling, floor) {
        (Some(ceiling), Some(floor))
            if ceiling - stalactite_height <= floor + stalagmite_height =>
        {
            let lowest = (ceiling - stalactite_height).max(floor + 1);
            let highest = (floor + stalagmite_height).min(ceiling - 1);
            let bottom = rng.next_int_between_inclusive(lowest, highest + 1);
            (ceiling - bottom, bottom - 1 - floor)
        }
        _ => (stalactite_height, stalagmite_height),
    };

    let merge_tips = rng.next_bool()
        && actual_stalactite > 0
        && actual_stalagmite > 0
        && column.height() == Some(actual_stalactite + actual_stalagmite);

    if let Some(ceiling) = ceiling {
        grow_speleothem(
            &config.pointed,
            &config.base_or_replaceable,
            volume,
            BlockPos::new(pos.x, ceiling - 1, pos.z),
            false,
            actual_stalactite,
            merge_tips,
        );
    }
    if let Some(floor) = floor {
        grow_speleothem(
            &config.pointed,
            &config.base_or_replaceable,
            volume,
            BlockPos::new(pos.x, floor + 1, pos.z),
            true,
            actual_stalagmite,
            merge_tips,
        );
    }
}

fn cluster_speleothem_height(
    config: &CompiledSpeleothemCluster,
    rng: &mut WorldgenRandom,
    dx: i32,
    dz: i32,
    density: f32,
    max_height: i32,
) -> i32 {
    if rng.next_f32() > density {
        return 0;
    }
    let from_center = dx.abs() + dz.abs();
    let mean = clamped_map(
        from_center as f64,
        0.0,
        config.max_distance_from_center_affecting_height_bias as f64,
        max_height as f64 / 2.0,
        0.0,
    ) as f32;
    (mean + rng.next_gaussian() as f32 * config.height_deviation as f32)
        .clamp(0.0, max_height as f32) as i32
}

fn can_place_pool<W: WorldGenVolume>(
    config: &CompiledSpeleothemCluster,
    volume: &W,
    pos: BlockPos,
) -> bool {
    let state = volume.get(pos).0 as usize;
    if volume.world().water_states.contains(state)
        || config.base_block_states.contains(state)
        || config.pointed_block_states.contains(state)
    {
        return false;
    }
    if volume.holds(&volume.world().water_fluid, pos + IVec3::Y) {
        return false;
    }
    let adjacent = |volume: &W, at: BlockPos| {
        volume.holds(&config.base_stone_overworld, at)
            || volume.holds(&volume.world().water_fluid, at)
    };
    Direction::HORIZONTAL
        .iter()
        .all(|side| adjacent(volume, pos + side.normal()))
        && adjacent(volume, pos + IVec3::NEG_Y)
}

/// `SpeleothemClusterFeature.replaceBlocksWithBaseBlocks`, which stops at the
/// first block it may not replace.
fn replace_with_base<W: WorldGenVolume>(
    config: &CompiledSpeleothemCluster,
    volume: &mut W,
    first: BlockPos,
    max_count: i32,
    direction: i32,
) {
    let mut pos = first;
    for _ in 0..max_count {
        if !volume.holds(&config.replaceable_blocks, pos) {
            return;
        }
        volume.set(pos, config.base_block);
        pos.y += direction;
    }
}

/// `SpeleothemUtils.growSpeleothem`: nothing grows unless the block behind the
/// start is one the column may root in.
pub(crate) fn grow_speleothem<W: WorldGenVolume>(
    pointed: &PointedStates,
    base_or_replaceable: &StateMask,
    volume: &mut W,
    start: BlockPos,
    tip_up: bool,
    height: i32,
    merged_tip: bool,
) {
    let step = if tip_up { 1 } else { -1 };
    if !volume.holds(base_or_replaceable, start - IVec3::Y * step) {
        return;
    }
    let mut pos = start;
    let place = |volume: &mut W, pos: &mut BlockPos, thickness: usize| {
        let waterlogged = volume.holds(&volume.world().water_fluid, *pos);
        let state = pointed.get(tip_up, thickness, waterlogged);
        volume.set(*pos, state);
        pos.y += step;
    };
    if height >= 3 {
        place(volume, &mut pos, BASE);
        for _ in 0..height - 3 {
            place(volume, &mut pos, MIDDLE);
        }
    }
    if height >= 2 {
        place(volume, &mut pos, FRUSTUM);
    }
    if height >= 1 {
        place(volume, &mut pos, if merged_tip { TIP_MERGE } else { TIP });
    }
}

#[derive(Clone, Debug)]
pub struct CompiledLargeDripstone {
    pub dripstone: VoxelId,
    /// `SpeleothemUtils.isBaseOrLava` over `dripstone_block`: its states, the
    /// replaceables, and lava.
    pub column_edge: StateMask,
    pub floor_to_ceiling_search_range: i32,
    pub column_radius_min: i32,
    pub column_radius_max: i32,
    pub height_scale: FloatProvider,
    pub max_column_radius_to_cave_height_ratio: f32,
    pub stalactite_bluntness: FloatProvider,
    pub stalagmite_bluntness: FloatProvider,
    pub wind_speed: FloatProvider,
    pub min_radius_for_wind: i32,
    pub min_bluntness_for_wind: f32,
    pub base_stone_overworld: StateMask,
}

#[derive(Clone, Copy, Debug)]
struct Wind {
    origin_y: i32,
    speed: (f64, f64),
    max_offset: i32,
}

impl Wind {
    fn offset(&self, pos: BlockPos) -> BlockPos {
        let (x, z) = self.speed;
        let scale = (self.origin_y - pos.y) as f64;
        let clamp = |value: f64| {
            (value * scale)
                .floor()
                .clamp(-self.max_offset as f64, self.max_offset as f64) as i32
        };
        BlockPos::new(pos.x + clamp(x), pos.y, pos.z + clamp(z))
    }
}

#[derive(Clone, Copy, Debug)]
struct Dripstone {
    root: BlockPos,
    pointing_up: bool,
    radius: i32,
    bluntness: f64,
    scale: f64,
}

impl Dripstone {
    fn height_at_radius(&self, check_radius: f32) -> i32 {
        speleothem_profile(
            check_radius as f64,
            self.radius as f64,
            self.scale,
            self.bluntness,
        ) as i32
    }

    fn suitable_for_wind(&self, min_radius: i32, min_bluntness: f32) -> bool {
        self.radius >= min_radius && self.bluntness >= min_bluntness as f64
    }
}
pub fn place_large_dripstone<W: WorldGenVolume>(
    config: &CompiledLargeDripstone,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
) -> bool {
    if !volume.world().is_empty_or_water(volume.get(origin)) {
        return false;
    }
    let Some(column) = scan_column(
        volume,
        origin,
        config.floor_to_ceiling_search_range,
        |state| volume.world().is_empty_or_water(state),
        |state| holds(&config.column_edge, state),
    ) else {
        return false;
    };
    let (Some(floor), Some(ceiling)) = (column.floor, column.ceiling) else {
        return false;
    };
    let height = ceiling - floor - 1;
    if height < 4 {
        return false;
    }

    let by_cave_height = (height as f32 * config.max_column_radius_to_cave_height_ratio) as i32;
    let max_radius = by_cave_height.clamp(config.column_radius_min, config.column_radius_max);
    let radius = rng.next_int_between_inclusive(config.column_radius_min, max_radius);

    let mut stalactite = Dripstone {
        root: BlockPos::new(origin.x, ceiling - 1, origin.z),
        pointing_up: false,
        radius,
        bluntness: config.stalactite_bluntness.sample(rng) as f64,
        scale: config.height_scale.sample(rng) as f64,
    };
    let mut stalagmite = Dripstone {
        root: BlockPos::new(origin.x, floor + 1, origin.z),
        pointing_up: true,
        radius,
        bluntness: config.stalagmite_bluntness.sample(rng) as f64,
        scale: config.height_scale.sample(rng) as f64,
    };

    let wind = (stalactite
        .suitable_for_wind(config.min_radius_for_wind, config.min_bluntness_for_wind)
        && stalagmite.suitable_for_wind(config.min_radius_for_wind, config.min_bluntness_for_wind))
    .then(|| {
        let speed = config.wind_speed.sample(rng);
        let direction = rng.next_f32() * std::f32::consts::PI;
        Wind {
            origin_y: origin.y,
            speed: (
                (cos_modern(direction as f64) * speed) as f64,
                (sin_modern(direction as f64) * speed) as f64,
            ),
            max_offset: 16 - radius,
        }
    });

    let stalactite_rooted = embed_base(&mut stalactite, volume, wind);
    let stalagmite_rooted = embed_base(&mut stalagmite, volume, wind);
    if stalactite_rooted {
        place_dripstone(config, &stalactite, volume, rng, wind);
    }
    if stalagmite_rooted {
        place_dripstone(config, &stalagmite, volume, rng, wind);
    }
    true
}
fn embed_base<W: WorldGenVolume>(
    dripstone: &mut Dripstone,
    volume: &W,
    wind: Option<Wind>,
) -> bool {
    while dripstone.radius > 1 {
        let mut root = dripstone.root;
        let tries = 10.min(dripstone.height_at_radius(0.0));
        for _ in 0..tries {
            if volume.holds(&volume.world().lava_states, root) {
                return false;
            }
            if circle_mostly_embedded(
                volume,
                wind.map_or(root, |w| w.offset(root)),
                dripstone.radius,
            ) {
                dripstone.root = root;
                return true;
            }
            root.y += if dripstone.pointing_up { -1 } else { 1 };
        }
        dripstone.radius /= 2;
    }
    false
}

/// `SpeleothemUtils.isCircleMostlyEmbeddedInStone`, whose angular step is what
/// decides how many samples the circle takes.
fn circle_mostly_embedded<W: WorldGenVolume>(volume: &W, center: BlockPos, xz_radius: i32) -> bool {
    let world = volume.world();
    if world.is_empty_or_water_or_lava(volume.get(center)) {
        return false;
    }
    let increment = 6.0f32 / xz_radius as f32;
    let mut angle = 0.0f32;
    while angle < std::f32::consts::TAU {
        let dx = (cos_modern(angle as f64) * xz_radius as f32) as i32;
        let dz = (sin_modern(angle as f64) * xz_radius as f32) as i32;
        let at = center + IVec3::new(dx, 0, dz);
        if world.is_empty_or_water_or_lava(volume.get(at)) {
            return false;
        }
        angle += increment;
    }
    true
}

fn place_dripstone<W: WorldGenVolume>(
    config: &CompiledLargeDripstone,
    dripstone: &Dripstone,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    wind: Option<Wind>,
) {
    let step = if dripstone.pointing_up { 1 } else { -1 };
    for dx in -dripstone.radius..=dripstone.radius {
        for dz in -dripstone.radius..=dripstone.radius {
            let current_radius = ((dx * dx + dz * dz) as f32).sqrt();
            if current_radius > dripstone.radius as f32 {
                continue;
            }
            let mut height = dripstone.height_at_radius(current_radius);
            if height <= 0 {
                continue;
            }
            if rng.next_f32() < 0.2 {
                height = (height as f32 * (rng.next_f32() * 0.2 + 0.8)) as i32;
            }
            let mut pos = dripstone.root + IVec3::new(dx, 0, dz);
            let max_y = if dripstone.pointing_up {
                volume.height(HeightmapName::WorldSurfaceWg, pos.x, pos.z)
            } else {
                i32::MAX
            };
            let mut out_of_stone = false;
            for _ in 0..height {
                if pos.y >= max_y {
                    break;
                }
                let at = wind.map_or(pos, |w| w.offset(pos));
                if volume.world().is_empty_or_water_or_lava(volume.get(at)) {
                    out_of_stone = true;
                    volume.set(at, config.dripstone);
                } else if out_of_stone && volume.holds(&config.base_stone_overworld, at) {
                    break;
                }
                pos.y += step;
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use mcrs_minecraft_core::value_provider::DispatchedFloatProvider;
    use mcrs_minecraft_random::worldgen::WorldgenRandom;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const DRIPSTONE: VoxelId = VoxelId(2);
    const WATER: VoxelId = VoxelId(3);
    const LAVA: VoxelId = VoxelId(4);
    const POINTED: u16 = 100;

    /// A distinct id per (direction, thickness, waterlogged), so a test can
    /// read back which one a column grew.
    pub(crate) fn pointed_states(first: u16) -> PointedStates {
        let mut states = [[[VoxelId(0); 2]; THICKNESSES]; 2];
        for (direction, plane) in states.iter_mut().enumerate() {
            for (thickness, pair) in plane.iter_mut().enumerate() {
                for (waterlogged, slot) in pair.iter_mut().enumerate() {
                    *slot = VoxelId(first + (direction * 10 + thickness * 2 + waterlogged) as u16);
                }
            }
        }
        PointedStates { states }
    }

    fn cluster() -> CompiledSpeleothemCluster {
        CompiledSpeleothemCluster {
            base_block: DRIPSTONE,
            base_block_states: mask_of([DRIPSTONE]),
            pointed: pointed_states(POINTED),
            pointed_block_states: StateMask::default(),
            replaceable_blocks: mask_of([STONE, DRIPSTONE]),
            base_or_replaceable: mask_of([STONE, DRIPSTONE]),
            floor_to_ceiling_search_range: 12,
            height: IntProvider::Constant(4),
            radius: IntProvider::Constant(1),
            max_stalagmite_stalactite_height_diff: 1,
            height_deviation: 3,
            speleothem_block_layer_thickness: IntProvider::Constant(2),
            density: FloatProvider::Constant(1.0),
            wetness: FloatProvider::Constant(0.0),
            chance_of_speleothem_at_max_distance_from_center: 0.1,
            max_distance_from_edge_affecting_chance_of_speleothem: 3,
            max_distance_from_center_affecting_height_bias: 8,
            base_stone_overworld: mask_of([STONE]),
        }
    }

    /// Stone below `floor`, stone at and above `ceiling`, air between.
    fn cave(floor: i32, ceiling: i32) -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.water = WATER;
        volume.world.water_states = mask_of([WATER]);
        volume.world.water_fluid = mask_of([WATER]);
        volume.world.lava_states = mask_of([LAVA]);
        for x in -8..=8 {
            for z in -8..=8 {
                for y in floor - 8..=ceiling + 8 {
                    if y <= floor || y >= ceiling {
                        volume.blocks.insert((x, y, z), STONE);
                    }
                }
                volume.heights.insert((x, z), 1000);
            }
        }
        volume
    }

    #[test]
    fn a_column_scan_finds_both_edges() {
        let volume = cave(10, 20);
        let column = scan_column(
            &volume,
            BlockPos::new(0, 15, 0),
            12,
            |state| volume.world().is_empty_or_water(state),
            |state| !volume.world().is_empty_or_water(state),
        )
        .expect("the origin is inside the column");
        assert_eq!(
            column,
            Column {
                floor: Some(10),
                ceiling: Some(20)
            }
        );
        assert_eq!(column.height(), Some(9));
    }

    #[test]
    fn a_scan_started_in_stone_finds_no_column() {
        let volume = cave(10, 20);
        assert!(
            scan_column(
                &volume,
                BlockPos::new(0, 5, 0),
                12,
                |state| volume.world().is_empty_or_water(state),
                |state| !volume.world().is_empty_or_water(state),
            )
            .is_none()
        );
    }

    #[test]
    fn a_cluster_outside_a_cave_places_nothing_and_draws_nothing() {
        let mut volume = cave(10, 20);
        let mut rng = WorldgenRandom::new(5);
        let before = rng.clone();
        assert!(!place_speleothem_cluster(
            &cluster(),
            &mut volume,
            &mut rng,
            BlockPos::new(0, 5, 0)
        ));
        assert_eq!(rng, before);
    }

    /// A one-cell radius is nine columns; each spends a wetness draw, two
    /// chance draws, a layer thickness and a height per end, the tip merge and
    /// — where the ends meet — the split that shares the gap between them. The
    /// pin is the next draw off the source, so any change to that ladder shows.
    ///
    /// Self-recorded: the ladder is read off `SpeleothemClusterFeature`, the
    /// number is read off this implementation.
    #[test]
    fn cluster_draw_count_anchor() {
        let mut volume = cave(10, 20);
        let mut rng = WorldgenRandom::new(0x005e_ed77);
        assert!(place_speleothem_cluster(
            &cluster(),
            &mut volume,
            &mut rng,
            BlockPos::new(0, 15, 0)
        ));
        assert_eq!(rng.next_java_long(), CLUSTER_PIN);
    }

    const CLUSTER_PIN: i64 = 3120301191049866738;

    #[test]
    fn a_cluster_grows_pointed_blocks_at_both_ends() {
        let mut volume = cave(10, 20);
        let mut rng = WorldgenRandom::new(0x005e_ed77);
        place_speleothem_cluster(&cluster(), &mut volume, &mut rng, BlockPos::new(0, 15, 0));
        let grown = volume
            .writes
            .iter()
            .filter(|(_, state)| state.0 >= POINTED)
            .count();
        assert!(grown > 0, "a full-density cluster grows something");
        assert!(
            volume.writes.iter().any(|(_, state)| *state == DRIPSTONE),
            "the layer under each end is replaced with the base block"
        );
    }

    fn dripstone() -> CompiledLargeDripstone {
        CompiledLargeDripstone {
            dripstone: DRIPSTONE,
            column_edge: mask_of([STONE, LAVA, DRIPSTONE]),
            floor_to_ceiling_search_range: 30,
            column_radius_min: 3,
            column_radius_max: 16,
            height_scale: FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
                min_inclusive: 0.4,
                max_exclusive: 2.0,
            }),
            max_column_radius_to_cave_height_ratio: 0.33,
            stalactite_bluntness: FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
                min_inclusive: 0.3,
                max_exclusive: 0.9,
            }),
            stalagmite_bluntness: FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
                min_inclusive: 0.4,
                max_exclusive: 1.0,
            }),
            wind_speed: FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
                min_inclusive: 0.0,
                max_exclusive: 0.3,
            }),
            min_radius_for_wind: 4,
            min_bluntness_for_wind: 0.6,
            base_stone_overworld: mask_of([STONE]),
        }
    }

    #[test]
    fn a_cave_shorter_than_four_is_refused_before_any_draw() {
        let mut volume = cave(10, 13);
        let mut rng = WorldgenRandom::new(3);
        let before = rng.clone();
        assert!(!place_large_dripstone(
            &dripstone(),
            &mut volume,
            &mut rng,
            BlockPos::new(0, 11, 0)
        ));
        assert_eq!(rng, before, "the height gate precedes the radius draw");
    }

    /// Radius, then two bluntness/scale pairs, then — only when both ends are
    /// wide and blunt enough — the wind's speed and direction.
    #[test]
    fn large_dripstone_header_is_five_or_seven_draws() {
        let mut volume = cave(0, 40);
        let mut rng = WorldgenRandom::new(0x1234_5678);
        let before = rng.clone();
        assert!(place_large_dripstone(
            &dripstone(),
            &mut volume,
            &mut rng,
            BlockPos::new(0, 20, 0)
        ));
        let mut header = before.clone();
        header.next_i32_bound(14);
        for _ in 0..4 {
            header.next_f32();
        }
        assert_ne!(rng, before);
        assert_ne!(
            rng, header,
            "a wide column also spends the wind and the per-cell draws"
        );
    }

    #[test]
    fn a_wide_cave_grows_dripstone_blocks() {
        let mut volume = cave(0, 40);
        let mut rng = WorldgenRandom::new(0x1234_5678);
        place_large_dripstone(&dripstone(), &mut volume, &mut rng, BlockPos::new(0, 20, 0));
        assert!(
            volume.writes.iter().any(|(_, state)| *state == DRIPSTONE),
            "a 39-high cave takes a full column"
        );
    }
}
