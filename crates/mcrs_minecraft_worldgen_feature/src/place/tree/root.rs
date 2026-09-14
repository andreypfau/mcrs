use crate::block_predicate::Direction;
use crate::placer::{StateMask, WorldGenVolume};
use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

use super::provider::StateProvider;
use super::trunk::{TreeContext, dist_manhattan};

#[derive(Clone, Debug)]
pub struct AboveRootPlacement {
    pub provider: StateProvider,
    pub chance: f32,
}

/// One `mangrove_root_placer` with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct MangroveRoots {
    pub trunk_offset_y: IntProvider,
    pub root_provider: StateProvider,
    pub above_root_placement: Option<AboveRootPlacement>,
    pub can_grow_through: StateMask,
    pub muddy_roots_in: StateMask,
    pub muddy_roots_provider: StateProvider,
    pub max_root_width: i32,
    pub max_root_length: i32,
    pub random_skew_chance: f32,
}

impl MangroveRoots {
    /// `RootPlacer.getTrunkOrigin`, drawn where the reference draws it: after
    /// the crown's radius and before the height bounds are checked.
    pub fn trunk_origin(&self, rng: &mut XoroshiroRandom, origin: BlockPos) -> BlockPos {
        origin + IVec3::Y * self.trunk_offset_y.sample(rng)
    }

    /// The roots as written, or `None` when the tree must refuse to place.
    pub fn place_roots<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        origin: BlockPos,
        trunk_origin: BlockPos,
    ) -> Option<Vec<BlockPos>> {
        let mut column = origin;
        while column.y < trunk_origin.y {
            if !self.can_place_root(cx, column) {
                return None;
            }
            column.y += 1;
        }

        let mut positions = vec![trunk_origin - IVec3::Y];
        for direction in Direction::HORIZONTAL {
            let side = trunk_origin + direction.normal();
            let mut branch = Vec::new();
            if !self.simulate_roots(cx, rng, side, direction, trunk_origin, &mut branch, 0) {
                return None;
            }
            positions.append(&mut branch);
            positions.push(side);
        }

        let mut written = Vec::with_capacity(positions.len());
        for pos in positions {
            self.place_root(cx, rng, pos, &mut written);
        }
        Some(written)
    }

    fn can_place_root<W: WorldGenVolume>(&self, cx: &TreeContext<'_, W>, pos: BlockPos) -> bool {
        cx.is_valid_tree_pos(pos) || cx.holds(&self.can_grow_through, pos)
    }

    fn simulate_roots<W: WorldGenVolume>(
        &self,
        cx: &TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        pos: BlockPos,
        direction: Direction,
        root_origin: BlockPos,
        branch: &mut Vec<BlockPos>,
        layer: i32,
    ) -> bool {
        if layer == self.max_root_length || branch.len() as i32 > self.max_root_length {
            return false;
        }
        for next in self
            .potential_root_positions(rng, pos, direction, root_origin)
            .into_iter()
            .flatten()
        {
            if self.can_place_root(cx, next) {
                branch.push(next);
                if !self.simulate_roots(cx, rng, next, direction, root_origin, branch, layer + 1) {
                    return false;
                }
            }
        }
        true
    }

    /// The one place a root draws while it is still being simulated: a skew
    /// roll, and past it a coin that decides between going out and going down.
    fn potential_root_positions(
        &self,
        rng: &mut XoroshiroRandom,
        pos: BlockPos,
        direction: Direction,
        root_origin: BlockPos,
    ) -> [Option<BlockPos>; 2] {
        let below = pos - IVec3::Y;
        let next_to = pos + direction.normal();
        let width = dist_manhattan(*pos, *root_origin);
        if width > self.max_root_width - 3 && width <= self.max_root_width {
            if rng.next_f32() < self.random_skew_chance {
                [Some(below), Some(next_to - IVec3::Y)]
            } else {
                [Some(below), None]
            }
        } else if width > self.max_root_width {
            [Some(below), None]
        } else if rng.next_f32() < self.random_skew_chance {
            [Some(below), None]
        } else if rng.next_bool() {
            [Some(next_to), None]
        } else {
            [Some(below), None]
        }
    }

    fn place_root<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        pos: BlockPos,
        written: &mut Vec<BlockPos>,
    ) {
        if cx.holds(&self.muddy_roots_in, pos) {
            let state = self.muddy_roots_provider.state(cx.volume, rng, pos);
            self.set(cx, written, pos, state);
            return;
        }
        if !self.can_place_root(cx, pos) {
            return;
        }
        let state = self.root_provider.state(cx.volume, rng, pos);
        self.set(cx, written, pos, state);
        let Some(above) = &self.above_root_placement else {
            return;
        };
        let up = pos + IVec3::Y;
        if rng.next_f32() < above.chance && cx.volume.is_air(up) {
            let state = above.provider.state(cx.volume, rng, up);
            self.set(cx, written, up, state);
        }
    }

    /// `getPotentiallyWaterloggedState` and the write in one: a state with no
    /// `waterlogged` property comes back unchanged.
    fn set<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        written: &mut Vec<BlockPos>,
        pos: BlockPos,
        state: VoxelId,
    ) {
        let wet = cx.holds(&cx.volume.world().water_fluid, pos);
        let state = cx.states.with_waterlogged(state, wet);
        written.push(pos);
        cx.volume.set(pos, state);
    }
}

#[cfg(test)]
mod tests {
    use crate::placer::mask_of;
    use mcrs_minecraft_chunk::Blocks;

    use mcrs_minecraft_core::value_provider::IntProvider;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::place::tree::provider::fake::{AIR, FakeVolume};
    use crate::place::tree::trunk::TreeStates;
    use crate::placer::WorldStates;

    const MUD: VoxelId = VoxelId(1);
    const STONE: VoxelId = VoxelId(2);
    const ROOT: VoxelId = VoxelId(3);
    const MUDDY_ROOT: VoxelId = VoxelId(4);
    const MOSS: VoxelId = VoxelId(5);

    fn placer() -> MangroveRoots {
        MangroveRoots {
            trunk_offset_y: IntProvider::Constant(1),
            root_provider: StateProvider::Simple(ROOT),
            above_root_placement: Some(AboveRootPlacement {
                provider: StateProvider::Simple(MOSS),
                chance: 0.5,
            }),
            can_grow_through: mask_of([MUD]),
            muddy_roots_in: mask_of([MUD]),
            muddy_roots_provider: StateProvider::Simple(MUDDY_ROOT),
            max_root_width: 8,
            max_root_length: 15,
            random_skew_chance: 0.2,
        }
    }

    fn states() -> TreeStates {
        TreeStates {
            valid_tree_pos: mask_of([AIR]),
            logs: StateMask::default(),
            air_or_leaves: mask_of([AIR]),
            persistent: StateMask::default(),
            ..TreeStates::default()
        }
    }

    /// Mud at y = 64 and below, air above, so a branch walking downward or
    /// outward is stopped by stone under the mud.
    fn volume() -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world = WorldStates {
            air_states: mask_of([AIR]),
            replaceable: mask_of([AIR]),
            solid_render: mask_of([STONE, MUD]),
            ..WorldStates::default()
        };
        for x in -16..=16 {
            for z in -16..=16 {
                volume.blocks.insert((x, 64, z), MUD);
                for y in 60..64 {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        volume
    }

    const ORIGIN: BlockPos = BlockPos::new(0, 65, 0);

    /// The trunk sits one above the origin, the roots run down and outward from
    /// it, and every root that landed in mud came out as the muddy state.
    #[test]
    fn the_roots_reach_the_mud_and_are_written_in_simulation_order() {
        let mut volume = volume();
        let placer = placer();
        let states = states();
        let provider = StateProvider::Simple(AIR);
        let mut cx = TreeContext::new(&mut volume, &states, &provider, &provider, &provider);

        let mut rng = XoroshiroRandom::new(99);
        let trunk_origin = placer.trunk_origin(&mut rng, ORIGIN);
        assert_eq!(trunk_origin, BlockPos::new(0, 66, 0));

        let roots = placer
            .place_roots(&mut cx, &mut rng, ORIGIN, trunk_origin)
            .expect("the column above the origin is free");

        assert_eq!(
            roots[0],
            trunk_origin - IVec3::Y,
            "the cell under the trunk is written first"
        );
        assert!(
            roots.len() >= 5,
            "one under the trunk and one per horizontal at least: {roots:?}"
        );
        for pos in &roots {
            let state = volume.get(*pos);
            assert!(
                matches!(state, ROOT | MUDDY_ROOT | MOSS),
                "{pos:?} carries {state:?}"
            );
        }
        assert!(
            roots.iter().any(|pos| pos.y == 64),
            "a branch reached the mud: {roots:?}"
        );
        assert!(
            roots.iter().all(|pos| pos.y <= 66),
            "nothing is written above the trunk's own cell but moss: {roots:?}"
        );
    }

    /// A blocked column between origin and trunk refuses the whole tree, and
    /// nothing is written on the way out.
    #[test]
    fn a_blocked_column_refuses_and_writes_nothing() {
        let mut volume = volume();
        volume.blocks.insert((0, 65, 0), STONE);
        let placer = placer();
        let states = states();
        let provider = StateProvider::Simple(AIR);
        let mut cx = TreeContext::new(&mut volume, &states, &provider, &provider, &provider);

        let mut rng = XoroshiroRandom::new(99);
        let before = rng.clone();
        assert!(
            placer
                .place_roots(&mut cx, &mut rng, ORIGIN, BlockPos::new(0, 67, 0))
                .is_none()
        );
        assert_eq!(rng, before, "the column check draws nothing");
        assert!(volume.writes.is_empty());
    }
}
