use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;
use rustc_hash::FxHashMap as HashMap;

use crate::feature::holds;

/// `WallSide`, in the property's declared order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WallSide {
    #[default]
    None,
    Low,
    Tall,
}

/// What a carpet state says about itself: whether it is the bottom layer, and
/// how far it climbs on each horizontal, in [`Direction::HORIZONTAL`] order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CarpetShape {
    pub base: bool,
    pub sides: [WallSide; 4],
}

impl CarpetShape {
    pub fn index(self) -> usize {
        self.sides
            .iter()
            .fold(usize::from(self.base), |acc, side| acc * 3 + *side as usize)
    }

    /// `MossyCarpetBlock.hasFaces`.
    fn has_faces(self) -> bool {
        self.base || self.sides.iter().any(|side| *side != WallSide::None)
    }
}

/// The number of `(base, north, east, south, west)` combinations, which is how
/// many states the block has.
pub const SHAPE_COUNT: usize = 2 * 3 * 3 * 3 * 3;

/// Every state of `minecraft:pale_moss_carpet`, resolved both ways, with the
/// neighbour test each side asks.
#[derive(Clone, Debug)]
pub struct MossyCarpetStates {
    /// Indexed by [`CarpetShape::index`].
    pub by_shape: [VoxelId; SHAPE_COUNT],
    /// The shape a state carries; a state of another block has no entry.
    pub by_state: HashMap<u16, CarpetShape>,
    /// Per [`Direction::HORIZONTAL`] entry, the states whose face toward the
    /// carpet is full — `MultifaceBlock.canAttachTo`.
    pub attaches: [StateMask; 4],
    pub air: VoxelId,
    /// The carpet's topper is the one shape the reference draws from the
    /// level's own random rather than from the feature's source, so it is not
    /// reproducible from the world seed there and the draw order here must not
    /// move. This build draws it from the position instead: same world every
    /// time it is generated, at the cost of a different topper than a vanilla
    /// save would hold.
    pub world_seed: i64,
}

impl MossyCarpetStates {
    /// Whether a state belongs to the carpet at all.
    pub fn holds(&self, state: VoxelId) -> bool {
        self.by_state.contains_key(&state.0)
    }

    fn state(&self, shape: CarpetShape) -> VoxelId {
        self.by_shape[shape.index()]
    }

    fn shape(&self, state: VoxelId) -> Option<CarpetShape> {
        self.by_state.get(&state.0).copied()
    }
}

/// `Mth.getSeed`.
fn position_seed(pos: BlockPos) -> i64 {
    let seed = (pos.x as i64).wrapping_mul(3_129_871)
        ^ (pos.z as i64).wrapping_mul(116_129_781)
        ^ pos.y as i64;
    let seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42_317_861)
        .wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

/// `MultifaceBlock.canAttachTo` for a horizontal, which is the only plane a
/// carpet climbs.
fn can_attach<W: WorldGenVolume>(
    states: &MossyCarpetStates,
    volume: &W,
    pos: BlockPos,
    face: usize,
) -> bool {
    let direction = Direction::HORIZONTAL[face];
    volume.holds(&states.attaches[face], pos + direction.normal())
}

/// `MossyCarpetBlock.getUpdatedState`.
fn updated<W: WorldGenVolume>(
    states: &MossyCarpetStates,
    volume: &W,
    mut shape: CarpetShape,
    pos: BlockPos,
    create_sides: bool,
) -> CarpetShape {
    let create_sides = create_sides || shape.base;
    let mut above = None;
    let mut below = None;

    for face in 0..4 {
        let mut side = if can_attach(states, volume, pos, face) {
            if create_sides {
                WallSide::Low
            } else {
                shape.sides[face]
            }
        } else {
            WallSide::None
        };

        if side == WallSide::Low {
            let up = *above.get_or_insert_with(|| states.shape(volume.get(pos + IVec3::Y)));
            if let Some(up) = up
                && up.sides[face] != WallSide::None
                && !up.base
            {
                side = WallSide::Tall;
            }

            if !shape.base {
                let down = *below.get_or_insert_with(|| states.shape(volume.get(pos - IVec3::Y)));
                if let Some(down) = down
                    && down.sides[face] == WallSide::None
                {
                    side = WallSide::None;
                }
            }
        }

        shape.sides[face] = side;
    }
    shape
}

/// `MossyCarpetBlock.createTopperWithSideChance`: `None` where the reference
/// answers with air.
fn topper<W: WorldGenVolume>(
    states: &MossyCarpetStates,
    volume: &W,
    pos: BlockPos,
    mut side_survives: impl FnMut() -> bool,
) -> Option<CarpetShape> {
    let above = pos + IVec3::Y;
    let previous = volume.get(above);
    let previous_shape = states.shape(previous);
    let carpet_above = previous_shape.is_some();

    let takes_a_topper = !previous_shape.is_some_and(|shape| shape.base)
        && (carpet_above || holds(&volume.world().replaceable, previous));
    if !takes_a_topper {
        return None;
    }

    let mut shape = updated(states, volume, CarpetShape::default(), above, true);
    for side in &mut shape.sides {
        if *side != WallSide::None && !side_survives() {
            *side = WallSide::None;
        }
    }

    (shape.has_faces() && states.state(shape) != previous).then_some(shape)
}

/// `MossyCarpetBlock.placeAt`: the carpet with the sides its neighbours allow,
/// then a topper above whose sides survive a coin each, then the carpet again
/// so its own sides can grow tall against the topper.
pub fn place_mossy_carpet<W: WorldGenVolume>(
    states: &MossyCarpetStates,
    volume: &mut W,
    at: BlockPos,
) {
    let base = CarpetShape {
        base: true,
        ..CarpetShape::default()
    };
    let placed = updated(states, volume, base, at, true);
    volume.set(at, states.state(placed));

    let mut rng = XoroshiroRandom::new((states.world_seed ^ position_seed(at)) as u64);
    let Some(topper) = topper(states, volume, at, || rng.next_bool()) else {
        return;
    };
    volume.set(at + IVec3::Y, states.state(topper));
    let settled = updated(states, volume, placed, at, true);
    volume.set(at, states.state(settled));
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen::feature::placer::{BoxRegion, WorldStates, mask_of};
    use mcrs_voxel_storage::{Blocks, BlocksMut};
    use std::sync::Arc;

    const ORIGIN: BlockPos = BlockPos::new(0, 0, 0);
    const AIR: VoxelId = VoxelId(0);
    const STONE: VoxelId = VoxelId(1);
    /// The carpet occupies `100..100 + SHAPE_COUNT`.
    const CARPET_BASE: u16 = 100;

    fn states(attaching: &[VoxelId]) -> MossyCarpetStates {
        let mut by_shape = [AIR; SHAPE_COUNT];
        let mut by_state = HashMap::default();
        for base in [false, true] {
            for n in 0..3 {
                for e in 0..3 {
                    for s in 0..3 {
                        for w in 0..3 {
                            let sides = [n, e, s, w].map(|value| match value {
                                0 => WallSide::None,
                                1 => WallSide::Low,
                                _ => WallSide::Tall,
                            });
                            let shape = CarpetShape { base, sides };
                            let id = VoxelId(CARPET_BASE + shape.index() as u16);
                            by_shape[shape.index()] = id;
                            by_state.insert(id.0, shape);
                        }
                    }
                }
            }
        }
        let attach: StateMask = mask_of(attaching.iter().map(|id| id.0));
        MossyCarpetStates {
            by_shape,
            by_state,
            attaches: [
                Arc::clone(&attach),
                Arc::clone(&attach),
                Arc::clone(&attach),
                attach,
            ],
            air: AIR,
            world_seed: 42,
        }
    }

    fn volume() -> BoxRegion {
        let mut volume = BoxRegion::new(BlockPos::new(-4, -4, -4), BlockPos::new(4, 4, 4), AIR);
        volume.world = WorldStates {
            air_states: mask_of([AIR]),
            replaceable: mask_of([AIR]),
            ..WorldStates::default()
        };
        volume
    }

    #[test]
    fn a_carpet_with_no_wall_beside_it_keeps_every_side_none() {
        let states = states(&[STONE]);
        let mut volume = volume();
        place_mossy_carpet(&states, &mut volume, ORIGIN);

        let shape = states.shape(volume.get(ORIGIN)).expect("a carpet");
        assert!(shape.base);
        assert_eq!(shape.sides, [WallSide::None; 4]);
        assert_eq!(
            volume.get(ORIGIN + IVec3::Y),
            AIR,
            "no side to climb means no topper"
        );
    }

    #[test]
    fn a_wall_to_the_north_grows_that_side_on_the_base() {
        let states = states(&[STONE]);
        let mut volume = volume();
        volume.set(ORIGIN + Direction::North.normal(), STONE);

        place_mossy_carpet(&states, &mut volume, ORIGIN);
        let shape = states.shape(volume.get(ORIGIN)).expect("a carpet");
        assert!(shape.base);
        assert_ne!(
            shape.sides[0],
            WallSide::None,
            "the north side climbs the wall"
        );
        assert_eq!(shape.sides[1..], [WallSide::None; 3]);
    }

    #[test]
    fn the_topper_only_grows_where_the_wall_reaches_above_the_carpet() {
        let states = states(&[STONE]);
        // A wall one block tall: beside the carpet but not beside the topper.
        let mut short = volume();
        short.set(ORIGIN + Direction::North.normal(), STONE);
        place_mossy_carpet(&states, &mut short, ORIGIN);
        assert_eq!(
            short.get(ORIGIN + IVec3::Y),
            AIR,
            "nothing for a topper to hold"
        );

        let mut tall = volume();
        tall.set(ORIGIN + Direction::North.normal(), STONE);
        tall.set(ORIGIN + Direction::North.normal() + IVec3::Y, STONE);
        place_mossy_carpet(&states, &mut tall, ORIGIN);
        let topper = states.shape(tall.get(ORIGIN + IVec3::Y));
        assert!(
            topper.is_some_and(|shape| !shape.base),
            "the topper is a sideways layer, never a base"
        );
    }

    #[test]
    fn the_shape_is_the_same_every_time_the_same_position_is_generated() {
        let states = states(&[STONE]);
        let shape_at = |pos: BlockPos| {
            let mut walled = volume();
            for face in 0..4 {
                let side = Direction::HORIZONTAL[face].normal();
                walled.set(pos + side, STONE);
                walled.set(pos + side + IVec3::Y, STONE);
            }
            place_mossy_carpet(&states, &mut walled, pos);
            (walled.get(pos), walled.get(pos + IVec3::Y))
        };
        assert_eq!(shape_at(ORIGIN), shape_at(ORIGIN));
    }

    #[test]
    fn every_shape_round_trips_through_its_state() {
        let states = states(&[]);
        for (state, shape) in &states.by_state {
            assert_eq!(states.state(*shape).0, *state);
        }
        assert_eq!(states.by_state.len(), SHAPE_COUNT);
    }
}
