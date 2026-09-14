use crate::feature::holds;
use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen::value_provider::IntProvider;

/// `BlockPos.withinBoxByManhattanDistance` cut at `max_depth`: shells of
/// ascending Manhattan distance, and inside a shell a mirrored `z` follows its
/// positive twin immediately.
///
/// Both callers stop at the first position past `max(reach)`, which the shell
/// order makes a bound on the depth rather than a test per position.
fn manhattan_ordered(reach: IVec3, max_depth: i32, mut visit: impl FnMut(IVec3)) {
    for depth in 0..=max_depth {
        let max_x = reach.x.min(depth);
        for x in -max_x..=max_x {
            let max_y = reach.y.min(depth - x.abs());
            for y in -max_y..=max_y {
                let z = depth - x.abs() - y.abs();
                if z > reach.z {
                    continue;
                }
                visit(IVec3::new(x, y, z));
                if z != 0 {
                    visit(IVec3::new(x, y, -z));
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompiledBlockBlob {
    pub state: VoxelId,
    pub can_place_on: Predicate,
}

/// `BlockBlobFeature`: three overlapping boxes, each drifting down and to the
/// north-west of the last.
pub fn place_block_blob<W: WorldGenVolume>(
    cfg: &CompiledBlockBlob,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let floor = volume.extent().min_y + 3;
    let mut origin = at;
    while origin.y > floor && !cfg.can_place_on.test(volume, origin - IVec3::Y) {
        origin.y -= 1;
    }
    if origin.y <= floor {
        return false;
    }

    for _ in 0..3 {
        let reach_x = rng.next_i32_bound(2);
        let reach_y = rng.next_i32_bound(2);
        let reach_z = rng.next_i32_bound(2);
        let threshold = (reach_x + reach_y + reach_z) as f32 * 0.333 + 0.5;
        for z in -reach_z..=reach_z {
            for y in -reach_y..=reach_y {
                for x in -reach_x..=reach_x {
                    if (x * x + y * y + z * z) as f64 <= (threshold * threshold) as f64 {
                        volume.set(
                            BlockPos::new(origin.x + x, origin.y + y, origin.z + z),
                            cfg.state,
                        );
                    }
                }
            }
        }
        origin += IVec3::new(
            -1 + rng.next_i32_bound(2),
            -rng.next_i32_bound(2),
            -1 + rng.next_i32_bound(2),
        );
    }
    true
}

#[derive(Clone, Debug)]
pub struct CompiledReplaceBlobs {
    /// Every state of the target block: the reference compares blocks, not
    /// states.
    pub target: StateMask,
    pub state: VoxelId,
    pub radius: IntProvider,
}

/// `ReplaceBlobsFeature`: fall down the column to the first target block, then
/// replace every target inside a Manhattan ball around it.
///
/// A column with no target spends no draw at all.
pub fn place_replace_blobs<W: WorldGenVolume>(
    cfg: &CompiledReplaceBlobs,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let extent = volume.extent();
    let bottom = extent.min_y + 1;
    let mut y = at.y.clamp(bottom, extent.min_y + extent.depth - 1);
    let center = loop {
        if y <= bottom {
            return false;
        }
        let candidate = BlockPos::new(at.x, y, at.z);
        if volume.holds(&cfg.target, candidate) {
            break candidate;
        }
        y -= 1;
    };

    let reach = IVec3::new(
        cfg.radius.sample(rng),
        cfg.radius.sample(rng),
        cfg.radius.sample(rng),
    );
    let mut replaced = false;
    manhattan_ordered(reach, reach.max_element(), |offset| {
        let pos = center + offset;
        if volume.holds(&cfg.target, pos) {
            volume.set(pos, cfg.state);
            replaced = true;
        }
    });
    replaced
}

#[derive(Clone, Debug)]
pub struct CompiledDelta {
    pub contents: VoxelId,
    /// Every state of the contents block, which is what the clearance test
    /// refuses to overwrite.
    pub contents_block: StateMask,
    pub rim: VoxelId,
    pub size: IntProvider,
    pub rim_size: IntProvider,
    pub cannot_replace: StateMask,
}

/// `DeltaFeature`: a flat Manhattan patch of lava with the rim laid down first
/// and the contents offset from it by the rim size.
pub fn place_delta<W: WorldGenVolume>(
    cfg: &CompiledDelta,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let spawn_rim = rng.next_f64() < 0.9;
    let rim_x = if spawn_rim {
        cfg.rim_size.sample(rng)
    } else {
        0
    };
    let rim_z = if spawn_rim {
        cfg.rim_size.sample(rng)
    } else {
        0
    };
    let has_rim = spawn_rim && rim_x != 0 && rim_z != 0;
    let radius_x = cfg.size.sample(rng);
    let radius_z = cfg.size.sample(rng);

    let mut placed = false;
    manhattan_ordered(
        IVec3::new(radius_x, 0, radius_z),
        radius_x.max(radius_z),
        |offset| {
            let pos = at + offset;
            if !is_clear(cfg, volume, pos) {
                return;
            }
            if has_rim {
                placed = true;
                volume.set(pos, cfg.rim);
            }
            let rimmed = pos + IVec3::new(rim_x, 0, rim_z);
            if is_clear(cfg, volume, rimmed) {
                placed = true;
                volume.set(rimmed, cfg.contents);
            }
        },
    );
    placed
}

/// Open sky above, solid on the five other faces, and nothing the delta refuses
/// to overwrite.
fn is_clear<W: WorldGenVolume>(cfg: &CompiledDelta, volume: &W, pos: BlockPos) -> bool {
    let state = volume.get(pos);
    if holds(&cfg.contents_block, state) || holds(&cfg.cannot_replace, state) {
        return false;
    }
    Direction::all()
        .into_iter()
        .all(|direction| volume.is_air(pos + direction.normal()) == (direction == Direction::Up))
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::value_provider::IntProvider;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const NETHERRACK: VoxelId = VoxelId(1);
    const BASALT: VoxelId = VoxelId(2);
    const STONE: VoxelId = VoxelId(3);
    const MOSSY: VoxelId = VoxelId(4);
    const LAVA: VoxelId = VoxelId(5);
    const MAGMA: VoxelId = VoxelId(6);
    const BEDROCK: VoxelId = VoxelId(7);

    fn seeded() -> XoroshiroRandom {
        XoroshiroRandom::new(0x0b10b)
    }

    fn solid_below(top: i32, state: VoxelId) -> FakeVolume {
        let mut volume = FakeVolume::default();
        for x in -12..=12 {
            for z in -12..=12 {
                for y in -8..=top {
                    volume.blocks.insert((x, y, z), state);
                }
            }
        }
        volume
    }

    /// The blob walks down to the first position its predicate accepts, then
    /// spends six draws per box: three reaches and three steps of the drift.
    #[test]
    fn a_block_blob_settles_on_the_ground_and_draws_six_per_box() {
        let cfg = CompiledBlockBlob {
            state: MOSSY,
            can_place_on: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([STONE]),
            },
        };
        let mut volume = solid_below(40, STONE);
        let mut rng = seeded();
        assert!(place_block_blob(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 50, 0)
        ));

        let mut replay = seeded();
        for _ in 0..3 {
            for _ in 0..6 {
                replay.next_i32_bound(2);
            }
        }
        assert_eq!(rng, replay, "three boxes of six draws each");

        assert!(
            volume.writes.iter().all(|(_, state)| *state == MOSSY),
            "the blob writes only its own state"
        );
        assert!(
            volume.writes.iter().any(|((_, y, _), _)| *y == 41),
            "the first box sits on the first accepted position: {:?}",
            volume.writes
        );
        assert!(
            volume
                .writes
                .iter()
                .all(|((x, y, z), _)| x.abs() <= 3 && z.abs() <= 3 && (36..=42).contains(y)),
            "the three boxes stay inside the drift: {:?}",
            volume.writes
        );
    }

    /// Nothing to stand on within the column is a refusal that spends no draw.
    #[test]
    fn a_block_blob_over_the_void_places_nothing() {
        let cfg = CompiledBlockBlob {
            state: MOSSY,
            can_place_on: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([STONE]),
            },
        };
        let mut volume = FakeVolume::default();
        let mut rng = seeded();
        let before = rng.clone();
        assert!(!place_block_blob(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 50, 0)
        ));
        assert!(volume.writes.is_empty());
        assert_eq!(rng, before);
    }

    /// The three radii are drawn only once a target has been found, and every
    /// replacement sits inside the Manhattan ball around it.
    #[test]
    fn replace_blobs_draws_three_radii_after_finding_its_target() {
        let cfg = CompiledReplaceBlobs {
            target: mask_of([NETHERRACK]),
            state: BASALT,
            radius: IntProvider::uniform(3, 7),
        };
        let mut volume = solid_below(40, NETHERRACK);
        let mut rng = seeded();
        assert!(place_replace_blobs(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 50, 0)
        ));

        let mut replay = seeded();
        let reach = IVec3::new(
            cfg.radius.sample(&mut replay),
            cfg.radius.sample(&mut replay),
            cfg.radius.sample(&mut replay),
        );
        assert_eq!(rng, replay, "one radius per axis, and nothing else");

        assert!(
            volume.writes.iter().all(|(_, state)| *state == BASALT),
            "every replacement is the blob's state"
        );
        assert!(
            volume.writes.iter().all(|((x, y, z), _)| {
                let manhattan = x.abs() + (y - 40).abs() + z.abs();
                manhattan <= reach.max_element()
                    && x.abs() <= reach.x
                    && (y - 40).abs() <= reach.y
                    && z.abs() <= reach.z
            }),
            "the ball is centred on the first netherrack below the origin"
        );
    }

    /// A column with no target refuses before the first draw.
    #[test]
    fn replace_blobs_without_a_target_draws_nothing() {
        let cfg = CompiledReplaceBlobs {
            target: mask_of([NETHERRACK]),
            state: BASALT,
            radius: IntProvider::uniform(3, 7),
        };
        let mut volume = solid_below(40, STONE);
        let mut rng = seeded();
        let before = rng.clone();
        assert!(!place_replace_blobs(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 50, 0)
        ));
        assert!(volume.writes.is_empty());
        assert_eq!(rng, before);
    }

    fn delta_config() -> CompiledDelta {
        CompiledDelta {
            contents: LAVA,
            contents_block: mask_of([LAVA]),
            rim: MAGMA,
            size: IntProvider::uniform(3, 7),
            rim_size: IntProvider::uniform(0, 2),
            cannot_replace: mask_of([BEDROCK]),
        }
    }

    /// One double for the rim coin, two rim sizes when it lands, then two
    /// sizes — the whole draw budget of a delta, spent before it looks at a
    /// single block.
    #[test]
    fn a_delta_spends_its_draws_before_it_reads_the_world() {
        let cfg = delta_config();
        let mut volume = solid_below(40, NETHERRACK);
        let mut rng = seeded();
        place_delta(&cfg, &mut volume, &mut rng, BlockPos::new(0, 40, 0));

        let mut replay = seeded();
        let spawn_rim = replay.next_f64() < 0.9;
        if spawn_rim {
            cfg.rim_size.sample(&mut replay);
            cfg.rim_size.sample(&mut replay);
        }
        cfg.size.sample(&mut replay);
        cfg.size.sample(&mut replay);
        assert_eq!(rng, replay, "one double, two rim sizes, two sizes");
    }

    /// The clearance test wants open sky over the position and rock on the five
    /// other faces, so a delta buried under stone writes nothing.
    #[test]
    fn a_buried_delta_writes_nothing() {
        let cfg = delta_config();
        let mut volume = solid_below(45, NETHERRACK);
        let mut rng = seeded();
        assert!(!place_delta(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 40, 0)
        ));
        assert!(volume.writes.is_empty());
    }

    /// On an open floor the patch fills in one flat layer, every cell of it
    /// either inside the Manhattan reach or the rim offset from a cell that is.
    #[test]
    fn a_delta_fills_an_open_floor_flat() {
        let cfg = delta_config();
        let mut volume = solid_below(40, NETHERRACK);
        let mut rng = seeded();
        assert!(place_delta(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 40, 0)
        ));

        let mut replay = seeded();
        let spawn_rim = replay.next_f64() < 0.9;
        let (rim_x, rim_z) = if spawn_rim {
            (
                cfg.rim_size.sample(&mut replay),
                cfg.rim_size.sample(&mut replay),
            )
        } else {
            (0, 0)
        };
        let radius_x = cfg.size.sample(&mut replay);
        let radius_z = cfg.size.sample(&mut replay);
        let limit = radius_x.max(radius_z);
        let inside = |x: i32, z: i32| {
            x.abs() + z.abs() <= limit && x.abs() <= radius_x && z.abs() <= radius_z
        };

        assert!(
            volume
                .writes
                .iter()
                .all(|(_, state)| *state == LAVA || *state == MAGMA),
            "a delta writes only its contents and its rim"
        );
        assert!(
            volume.writes.iter().all(|((_, y, _), _)| *y == 40),
            "a delta is flat"
        );
        assert!(
            volume
                .writes
                .iter()
                .all(|((x, _, z), _)| inside(*x, *z) || inside(*x - rim_x, *z - rim_z)),
            "every write is a patch cell or the rim offset from one: {:?}",
            volume.writes
        );
    }
}
