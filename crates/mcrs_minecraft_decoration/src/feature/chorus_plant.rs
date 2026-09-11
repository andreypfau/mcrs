use crate::feature::holds;
use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};
use mcrs_voxel_storage::VoxelId;

const MAX_HORIZONTAL_SPREAD: i32 = 8;
const MAX_BRANCHING_DEPTH: i32 = 4;

#[derive(Clone, Debug)]
pub struct CompiledChorusPlant {
    /// `supports_chorus_plant`, which only the downward connection reads.
    pub supports: StateMask,
    /// Every state of `chorus_plant` and of `chorus_flower`: what a neighbour
    /// must be for a connection to point at it.
    pub plant_or_flower: StateMask,
    /// The 64 states of `chorus_plant`, indexed by [`connection_index`].
    pub plant_by_connections: [VoxelId; 64],
    pub flower_age5: VoxelId,
}

/// `down`, `up`, `north`, `east`, `south`, `west` as bits 5 down to 0.
pub fn connection_index(
    down: bool,
    up: bool,
    north: bool,
    east: bool,
    south: bool,
    west: bool,
) -> usize {
    (down as usize) << 5
        | (up as usize) << 4
        | (north as usize) << 3
        | (east as usize) << 2
        | (south as usize) << 1
        | (west as usize)
}

/// A chorus plant grown from an air cell over end stone.
pub fn place_chorus_plant<W: WorldGenVolume>(
    cfg: &CompiledChorusPlant,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: IVec3,
) -> bool {
    if !volume.is_air(at) || !volume.holds(&cfg.supports, at - IVec3::Y) {
        return false;
    }
    connect(cfg, volume, at);
    grow(cfg, volume, rng, at, at, 0);
    true
}

fn grow<W: WorldGenVolume>(
    cfg: &CompiledChorusPlant,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    current: IVec3,
    start: IVec3,
    depth: i32,
) {
    let mut height = rng.next_i32_bound(4) + 1;
    if depth == 0 {
        height += 1;
    }

    for step in 1..=height {
        let target = IVec3::new(current.x, current.y + step, current.z);
        // A blocked column abandons the whole node, flower included.
        if !neighbours_empty(volume, target, None) {
            return;
        }
        connect(cfg, volume, target);
        connect(cfg, volume, target - IVec3::Y);
    }

    let mut placed_stem = false;
    if depth < MAX_BRANCHING_DEPTH {
        let mut stems = rng.next_i32_bound(4);
        if depth == 0 {
            stems += 1;
        }
        for _ in 0..stems {
            let facing = rng.next_i32_bound(4) as usize;
            let target = current + Direction::HORIZONTAL[facing].normal() + IVec3::Y * height;
            let below = target - IVec3::Y;
            if (target.x - start.x).abs() < MAX_HORIZONTAL_SPREAD
                && (target.z - start.z).abs() < MAX_HORIZONTAL_SPREAD
                && volume.is_air(target)
                && volume.is_air(below)
                && neighbours_empty(volume, target, Some(facing ^ 2))
            {
                placed_stem = true;
                connect(cfg, volume, target);
                connect(cfg, volume, IVec3::new(current.x, target.y, current.z));
                grow(cfg, volume, rng, target, start, depth + 1);
            }
        }
    }

    if !placed_stem {
        let tip = IVec3::new(current.x, current.y + height, current.z);
        volume.set(tip, cfg.flower_age5);
    }
}

fn neighbours_empty<W: WorldGenVolume>(volume: &W, pos: IVec3, ignore: Option<usize>) -> bool {
    Direction::HORIZONTAL
        .iter()
        .enumerate()
        .all(|(index, side)| ignore == Some(index) || volume.is_air(pos + side.normal()))
}

fn connect<W: WorldGenVolume>(cfg: &CompiledChorusPlant, volume: &mut W, pos: IVec3) {
    let joins = |volume: &W, x: i32, y: i32, z: i32| {
        volume.holds(&cfg.plant_or_flower, IVec3::new(x, y, z))
    };
    let below = volume.get(pos - IVec3::Y);
    let down = holds(&cfg.plant_or_flower, below) || holds(&cfg.supports, below);
    let index = connection_index(
        down,
        joins(volume, pos.x, pos.y + 1, pos.z),
        joins(volume, pos.x, pos.y, pos.z - 1),
        joins(volume, pos.x + 1, pos.y, pos.z),
        joins(volume, pos.x, pos.y, pos.z + 1),
        joins(volume, pos.x - 1, pos.y, pos.z),
    );
    volume.set(pos, cfg.plant_by_connections[index]);
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const END_STONE: VoxelId = VoxelId(1);
    /// The 64 connection states of `chorus_plant` sit at 64..128, the flower at
    /// 128, so a test reads a written connection mask straight off the id.
    const PLANT_BASE: u16 = 64;
    const FLOWER: VoxelId = VoxelId(128);
    const ORIGIN: IVec3 = IVec3::new(8, 70, 8);

    fn config() -> CompiledChorusPlant {
        let mut plant_by_connections = [VoxelId(0); 64];
        for (index, slot) in plant_by_connections.iter_mut().enumerate() {
            *slot = VoxelId(PLANT_BASE + index as u16);
        }
        CompiledChorusPlant {
            supports: mask_of([END_STONE.0]),
            plant_or_flower: mask_of((PLANT_BASE..PLANT_BASE + 64).chain([FLOWER.0])),
            plant_by_connections,
            flower_age5: FLOWER,
        }
    }

    fn island() -> FakeVolume {
        let mut volume = FakeVolume::default();
        for x in ORIGIN.x - 12..=ORIGIN.x + 12 {
            for z in ORIGIN.z - 12..=ORIGIN.z + 12 {
                volume.blocks.insert((x, ORIGIN.y - 1, z), END_STONE);
            }
        }
        volume
    }

    /// Neither air nor ground costs a draw.
    #[test]
    fn a_refused_origin_draws_nothing() {
        let cfg = config();
        let mut volume = island();
        volume
            .blocks
            .insert((ORIGIN.x, ORIGIN.y, ORIGIN.z), END_STONE);
        let mut rng = XoroshiroRandom::new(3);
        let before = rng.clone();
        assert!(!place_chorus_plant(&cfg, &mut volume, &mut rng, ORIGIN));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());

        let mut bare = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(3);
        assert!(!place_chorus_plant(&cfg, &mut bare, &mut rng, ORIGIN));
        assert_eq!(rng, before);
    }

    /// A neighbour touching the first cell above the origin abandons the node
    /// before the branch draws and before the flower: one draw, one write, and
    /// that write is the root with its `down` connection set.
    #[test]
    fn a_blocked_column_spends_one_draw_and_writes_only_the_root() {
        let cfg = config();
        let mut volume = island();
        volume
            .blocks
            .insert((ORIGIN.x + 1, ORIGIN.y + 1, ORIGIN.z), END_STONE);
        let mut rng = XoroshiroRandom::new(0x00c0_ffee);
        assert!(place_chorus_plant(&cfg, &mut volume, &mut rng, ORIGIN));

        let mut replay = XoroshiroRandom::new(0x00c0_ffee);
        replay.next_i32_bound(4);
        assert_eq!(
            rng, replay,
            "the height, and nothing after the early return"
        );
        assert_eq!(volume.writes.len(), 1);
        assert_eq!(
            volume.writes[0],
            (
                (ORIGIN.x, ORIGIN.y, ORIGIN.z),
                VoxelId(
                    PLANT_BASE + connection_index(true, false, false, false, false, false) as u16
                )
            )
        );
    }

    /// Open sky: the plant stays inside the reference's spread, roots itself on
    /// the end stone and tops every unbranched node with a flower.
    #[test]
    fn an_open_plant_stays_within_the_spread_and_flowers() {
        let cfg = config();
        for seed in 0..64u64 {
            let mut volume = island();
            let mut rng = XoroshiroRandom::new(seed);
            assert!(place_chorus_plant(&cfg, &mut volume, &mut rng, ORIGIN));
            assert!(
                volume.writes.iter().any(|(_, state)| *state == FLOWER),
                "seed {seed} grew no flower"
            );
            for ((x, y, z), _) in &volume.writes {
                assert!(
                    (x - ORIGIN.x).abs() <= MAX_HORIZONTAL_SPREAD
                        && (z - ORIGIN.z).abs() <= MAX_HORIZONTAL_SPREAD,
                    "seed {seed} wrote at {x},{y},{z}, outside the spread"
                );
            }
        }
    }
}
