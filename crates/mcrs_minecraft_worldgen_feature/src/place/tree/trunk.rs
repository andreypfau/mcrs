use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::block_predicate::Direction;
use crate::placer::{StateMask, WorldGenVolume};
use crate::tree::{TrunkPlacer, UniformIntRange, UnitFloat};
use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

use super::provider::StateProvider;

use mcrs_minecraft_core::BlockPos;
pub use mcrs_minecraft_core::{Axis, dist_manhattan};

pub fn random_horizontal(rng: &mut XoroshiroRandom) -> Direction {
    Direction::HORIZONTAL[rng.next_i32_bound(Direction::HORIZONTAL.len() as i32) as usize]
}

/// `Direction.allShuffled`, which is `Util.shuffle` over all six: a descending
/// Fisher-Yates spending `n - 1` draws, a slot swapped with itself as readily
/// as with any other.
pub fn all_shuffled(rng: &mut XoroshiroRandom) -> [Direction; 6] {
    let mut faces = Direction::all();
    mcrs_minecraft_random::shuffle(&mut faces, rng);
    faces
}

/// Every block set and state edit the trunk and foliage passes name, resolved
/// once at freeze so nothing here looks a block up by id.
#[derive(Clone, Debug, Default)]
pub struct TreeStates {
    /// `TreeFeature.validTreePos`: air, or in `#replaceable_by_trees`.
    pub valid_tree_pos: StateMask,
    /// `#logs`, which widens `validTreePos` into `TrunkPlacer.isFree`.
    pub logs: StateMask,
    /// Air plus `#leaves`, which is `TreeFeature.isAirOrLeaves`.
    pub air_or_leaves: StateMask,
    /// States whose `persistent` is true, which stops a leaf.
    pub persistent: StateMask,
    /// `trySetValue(RotatedPillarBlock.AXIS, …)`: the state's block as X, Y, Z.
    pub axis: HashMap<u16, [VoxelId; 3]>,
    /// `setValue(WATERLOGGED, …)`: `[dry, wet]` per state of a waterloggable
    /// block. A state absent here carries no such property.
    pub waterlogged: HashMap<u16, [VoxelId; 2]>,
}

impl TreeStates {
    pub fn with_axis(&self, state: VoxelId, axis: Axis) -> VoxelId {
        match self.axis.get(&state.0) {
            Some(by_axis) => by_axis[axis as usize],
            None => state,
        }
    }

    pub fn with_waterlogged(&self, state: VoxelId, waterlogged: bool) -> VoxelId {
        match self.waterlogged.get(&state.0) {
            Some(pair) => pair[usize::from(waterlogged)],
            None => state,
        }
    }
}

/// The volume a tree grows in, the tree's own tables, and the positions it has
/// written so far — which the leaf relaxation and every decorator read back.
pub struct TreeContext<'a, W> {
    pub volume: &'a mut W,
    pub states: &'a TreeStates,
    pub trunk_provider: &'a StateProvider,
    pub foliage_provider: &'a StateProvider,
    pub below_trunk_provider: &'a StateProvider,
    pub logs: Vec<BlockPos>,
    pub leaves: Vec<BlockPos>,
    leaf_set: HashSet<BlockPos>,
}

impl<'a, W: WorldGenVolume> TreeContext<'a, W> {
    pub fn new(
        volume: &'a mut W,
        states: &'a TreeStates,
        trunk_provider: &'a StateProvider,
        foliage_provider: &'a StateProvider,
        below_trunk_provider: &'a StateProvider,
    ) -> Self {
        TreeContext {
            volume,
            states,
            trunk_provider,
            foliage_provider,
            below_trunk_provider,
            logs: Vec::new(),
            leaves: Vec::new(),
            leaf_set: HashSet::default(),
        }
    }

    pub fn get(&self, pos: BlockPos) -> VoxelId {
        self.volume.get(pos)
    }

    pub fn holds(&self, mask: &StateMask, pos: BlockPos) -> bool {
        self.volume.holds(mask, pos)
    }

    pub fn is_valid_tree_pos(&self, pos: BlockPos) -> bool {
        self.holds(&self.states.valid_tree_pos, pos)
    }

    pub fn is_air_or_leaves(&self, pos: BlockPos) -> bool {
        self.holds(&self.states.air_or_leaves, pos)
    }

    /// `TrunkPlacer.isFree` before a placer's own widening.
    pub fn is_free(&self, pos: BlockPos) -> bool {
        self.is_valid_tree_pos(pos) || self.holds(&self.states.logs, pos)
    }

    pub fn is_persistent(&self, pos: BlockPos) -> bool {
        self.holds(&self.states.persistent, pos)
    }

    /// `level.isFluidAtPosition(pos, fluid -> fluid.isSourceOfType(WATER))`.
    pub fn is_water_source(&self, pos: BlockPos) -> bool {
        self.holds(&self.volume.world().water_source, pos)
    }

    fn set(&mut self, pos: BlockPos, state: VoxelId) {
        self.volume.set(pos, state);
    }

    pub fn set_log(&mut self, pos: BlockPos, state: VoxelId) {
        self.logs.push(pos);
        self.set(pos, state);
    }

    pub fn set_leaf(&mut self, pos: BlockPos, state: VoxelId) {
        if self.leaf_set.insert(pos) {
            self.leaves.push(pos);
        }
        self.set(pos, state);
    }

    /// `FoliageSetter.isSet`: this tree's own leaves, not the world's.
    pub fn is_leaf_set(&self, pos: BlockPos) -> bool {
        self.leaf_set.contains(&pos)
    }

    /// `TrunkPlacer.placeBelowTrunkBlock`, the only user of a provider's
    /// optional form: nothing matching writes nothing at all.
    pub fn place_below_trunk_block(&mut self, rng: &mut XoroshiroRandom, pos: BlockPos) {
        if let Some(state) = self
            .below_trunk_provider
            .optional_state(self.volume, rng, pos)
        {
            self.set(pos, state);
        }
    }
}

/// Where a crown hangs off a trunk, as the trunk placer left it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoliageAttachment {
    pub pos: BlockPos,
    pub radius_offset_xz: i32,
    pub double_trunk: bool,
}

impl FoliageAttachment {
    pub const fn new(pos: BlockPos, radius_offset_xz: i32, double_trunk: bool) -> Self {
        FoliageAttachment {
            pos,
            radius_offset_xz,
            double_trunk,
        }
    }
}

#[derive(Clone, Copy)]
struct CherryBranch<'a> {
    horizontal_length: &'a IntProvider,
    end_offset_from_top: &'a IntProvider,
}

/// A trunk placer beside the one block set it can name resolved:
/// `upwards_branching`'s `can_grow_through`, `None` for every other placer.
#[derive(Clone, Debug)]
pub struct Trunk {
    pub placer: TrunkPlacer,
    pub grow_through: Option<StateMask>,
}

impl Trunk {
    /// `base_height`, `height_rand_a` and `height_rand_b`, which every placer
    /// carries and draws in that order.
    pub fn base(&self) -> (i32, i32, i32) {
        use TrunkPlacer::*;
        match &self.placer {
            Straight {
                base_height,
                height_rand_a,
                height_rand_b,
            }
            | Forking {
                base_height,
                height_rand_a,
                height_rand_b,
            }
            | Giant {
                base_height,
                height_rand_a,
                height_rand_b,
            }
            | MegaJungle {
                base_height,
                height_rand_a,
                height_rand_b,
            }
            | DarkOak {
                base_height,
                height_rand_a,
                height_rand_b,
            }
            | Fancy {
                base_height,
                height_rand_a,
                height_rand_b,
            }
            | Bending {
                base_height,
                height_rand_a,
                height_rand_b,
                ..
            }
            | UpwardsBranching {
                base_height,
                height_rand_a,
                height_rand_b,
                ..
            }
            | Cherry {
                base_height,
                height_rand_a,
                height_rand_b,
                ..
            }
            | Poplar {
                base_height,
                height_rand_a,
                height_rand_b,
                ..
            } => (base_height.0, height_rand_a.0, height_rand_b.0),
        }
    }

    pub fn tree_height(&self, rng: &mut XoroshiroRandom) -> i32 {
        let (base_height, height_rand_a, height_rand_b) = self.base();
        base_height + rng.next_i32_bound(height_rand_a + 1) + rng.next_i32_bound(height_rand_b + 1)
    }

    /// `validTreePos`, which only `upwards_branching` widens.
    pub fn valid_tree_pos<W: WorldGenVolume>(
        &self,
        cx: &TreeContext<'_, W>,
        pos: BlockPos,
    ) -> bool {
        cx.is_valid_tree_pos(pos)
            || self
                .grow_through
                .as_ref()
                .is_some_and(|mask| cx.holds(mask, pos))
    }

    /// `isFree`, which the tree's own free-space scan runs over its footprint.
    pub fn is_free<W: WorldGenVolume>(&self, cx: &TreeContext<'_, W>, pos: BlockPos) -> bool {
        self.valid_tree_pos(cx, pos) || cx.is_free(pos)
    }

    fn place_log<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        pos: BlockPos,
        axis: Option<Axis>,
    ) -> bool {
        if !self.valid_tree_pos(cx, pos) {
            return false;
        }
        let mut state = cx.trunk_provider.state(cx.volume, rng, pos);
        if let Some(axis) = axis {
            state = cx.states.with_axis(state, axis);
        }
        cx.set_log(pos, state);
        true
    }

    fn place_log_if_free<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        pos: BlockPos,
    ) {
        if self.is_free(cx, pos) {
            self.place_log(cx, rng, pos, None);
        }
    }

    pub fn place_trunk<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
    ) -> Vec<FoliageAttachment> {
        match &self.placer {
            TrunkPlacer::Straight { .. } => {
                cx.place_below_trunk_block(rng, origin - IVec3::Y);
                for y in 0..tree_height {
                    self.place_log(cx, rng, origin + IVec3::Y * y, None);
                }
                vec![FoliageAttachment::new(
                    origin + IVec3::Y * tree_height,
                    0,
                    false,
                )]
            }
            TrunkPlacer::Forking { .. } => self.place_forking(cx, rng, tree_height, origin),
            TrunkPlacer::Giant { .. } => self.place_giant(cx, rng, tree_height, origin),
            TrunkPlacer::MegaJungle { .. } => self.place_mega_jungle(cx, rng, tree_height, origin),
            TrunkPlacer::DarkOak { .. } => self.place_dark_oak(cx, rng, tree_height, origin),
            TrunkPlacer::Fancy { .. } => self.place_fancy(cx, rng, tree_height, origin),
            TrunkPlacer::Bending {
                min_height_for_leaves,
                bend_length,
                ..
            } => self.place_bending(
                cx,
                rng,
                tree_height,
                origin,
                min_height_for_leaves,
                bend_length,
            ),
            TrunkPlacer::UpwardsBranching {
                extra_branch_steps,
                place_branch_per_log_probability,
                extra_branch_length,
                ..
            } => self.place_upwards_branching(
                cx,
                rng,
                tree_height,
                origin,
                extra_branch_steps,
                place_branch_per_log_probability,
                extra_branch_length,
            ),
            TrunkPlacer::Cherry {
                branch_count,
                branch_horizontal_length,
                branch_start_offset_from_top,
                branch_end_offset_from_top,
                ..
            } => self.place_cherry(
                cx,
                rng,
                tree_height,
                origin,
                branch_count,
                branch_start_offset_from_top,
                CherryBranch {
                    horizontal_length: branch_horizontal_length,
                    end_offset_from_top: branch_end_offset_from_top,
                },
            ),
            TrunkPlacer::Poplar {
                trunk_height_above_branches,
                branch_amount,
                ..
            } => self.place_poplar(
                cx,
                rng,
                tree_height,
                origin,
                trunk_height_above_branches,
                branch_amount,
            ),
        }
    }

    fn place_forking<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
    ) -> Vec<FoliageAttachment> {
        cx.place_below_trunk_block(rng, origin - IVec3::Y);
        let mut attachments = Vec::new();
        let lean_direction = random_horizontal(rng);
        let lean_height = tree_height - rng.next_i32_bound(4) - 1;
        let mut lean_steps = 3 - rng.next_i32_bound(3);
        let (mut tx, mut tz) = (origin.x, origin.z);
        let mut end_y = None;

        for yo in 0..tree_height {
            let yy = origin.y + yo;
            if yo >= lean_height && lean_steps > 0 {
                let delta = lean_direction.normal();
                tx += delta.x;
                tz += delta.z;
                lean_steps -= 1;
            }
            if self.place_log(cx, rng, BlockPos::new(tx, yy, tz), None) {
                end_y = Some(yy + 1);
            }
        }

        if let Some(y) = end_y {
            attachments.push(FoliageAttachment::new(BlockPos::new(tx, y, tz), 1, false));
        }

        tx = origin.x;
        tz = origin.z;
        let branch_direction = random_horizontal(rng);
        if branch_direction != lean_direction {
            let mut yo = lean_height - rng.next_i32_bound(2) - 1;
            let mut branch_steps = 1 + rng.next_i32_bound(3);
            end_y = None;

            while yo < tree_height && branch_steps > 0 {
                if yo >= 1 {
                    let yyx = origin.y + yo;
                    let delta = branch_direction.normal();
                    tx += delta.x;
                    tz += delta.z;
                    if self.place_log(cx, rng, BlockPos::new(tx, yyx, tz), None) {
                        end_y = Some(yyx + 1);
                    }
                }
                yo += 1;
                branch_steps -= 1;
            }

            if let Some(y) = end_y {
                attachments.push(FoliageAttachment::new(BlockPos::new(tx, y, tz), 0, false));
            }
        }

        attachments
    }

    fn place_giant<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
    ) -> Vec<FoliageAttachment> {
        let below = origin - IVec3::Y;
        cx.place_below_trunk_block(rng, below);
        cx.place_below_trunk_block(rng, below + IVec3::X);
        cx.place_below_trunk_block(rng, below + IVec3::Z);
        cx.place_below_trunk_block(rng, below + IVec3::X + IVec3::Z);

        for hh in 0..tree_height {
            self.place_log_if_free(cx, rng, origin + IVec3::new(0, hh, 0));
            if hh < tree_height - 1 {
                self.place_log_if_free(cx, rng, origin + IVec3::new(1, hh, 0));
                self.place_log_if_free(cx, rng, origin + IVec3::new(1, hh, 1));
                self.place_log_if_free(cx, rng, origin + IVec3::new(0, hh, 1));
            }
        }

        vec![FoliageAttachment::new(
            origin + IVec3::Y * tree_height,
            0,
            true,
        )]
    }

    fn place_mega_jungle<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
    ) -> Vec<FoliageAttachment> {
        let mut attachments = self.place_giant(cx, rng, tree_height, origin);
        let mut branch_height = tree_height - 2 - rng.next_i32_bound(4);

        while branch_height > tree_height / 2 {
            let angle = rng.next_f32() * std::f32::consts::TAU;
            let mut bx = 0;
            let mut bz = 0;

            for b in 0..5 {
                bx = (1.5 + mcrs_minecraft_core::mth::cos_modern(angle as f64) * b as f32) as i32;
                bz = (1.5 + mcrs_minecraft_core::mth::sin_modern(angle as f64) * b as f32) as i32;
                let at = origin + IVec3::new(bx, branch_height - 3 + b / 2, bz);
                self.place_log(cx, rng, at, None);
            }

            attachments.push(FoliageAttachment::new(
                origin + IVec3::new(bx, branch_height, bz),
                -2,
                false,
            ));
            branch_height -= 2 + rng.next_i32_bound(4);
        }

        attachments
    }

    fn place_dark_oak<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
    ) -> Vec<FoliageAttachment> {
        let mut attachments = Vec::new();
        let below = origin - IVec3::Y;
        cx.place_below_trunk_block(rng, below);
        cx.place_below_trunk_block(rng, below + IVec3::X);
        cx.place_below_trunk_block(rng, below + IVec3::Z);
        cx.place_below_trunk_block(rng, below + IVec3::X + IVec3::Z);

        let lean_direction = random_horizontal(rng);
        let lean_height = tree_height - rng.next_i32_bound(4);
        let mut lean_steps = 2 - rng.next_i32_bound(3);
        let (mut tx, mut tz) = (origin.x, origin.z);
        let end_y = origin.y + tree_height - 1;

        for dy in 0..tree_height {
            if dy >= lean_height && lean_steps > 0 {
                let delta = lean_direction.normal();
                tx += delta.x;
                tz += delta.z;
                lean_steps -= 1;
            }
            let at = BlockPos::new(tx, origin.y + dy, tz);
            if cx.is_air_or_leaves(at) {
                self.place_log(cx, rng, at, None);
                self.place_log(cx, rng, at + IVec3::X, None);
                self.place_log(cx, rng, at + IVec3::Z, None);
                self.place_log(cx, rng, at + IVec3::X + IVec3::Z, None);
            }
        }

        attachments.push(FoliageAttachment::new(
            BlockPos::new(tx, end_y, tz),
            0,
            true,
        ));

        for ox in -1..=2 {
            for oz in -1..=2 {
                if (!(0..=1).contains(&ox) || !(0..=1).contains(&oz)) && rng.next_i32_bound(3) <= 0
                {
                    let length = rng.next_i32_bound(3) + 2;
                    for branch_y in 0..length {
                        self.place_log(
                            cx,
                            rng,
                            BlockPos::new(origin.x + ox, end_y - branch_y - 1, origin.z + oz),
                            None,
                        );
                    }
                    attachments.push(FoliageAttachment::new(
                        BlockPos::new(origin.x + ox, end_y, origin.z + oz),
                        0,
                        false,
                    ));
                }
            }
        }

        attachments
    }

    fn place_fancy<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
    ) -> Vec<FoliageAttachment> {
        let height = tree_height + 2;
        let trunk_height = (height as f64 * 0.618).floor() as i32;
        cx.place_below_trunk_block(rng, origin - IVec3::Y);
        let clusters_per_y = 1.min((1.382 + (height as f64 / 13.0).powf(2.0)).floor() as i32);
        let trunk_top = origin.y + trunk_height;

        let mut foliage_coords = vec![(
            FoliageAttachment::new(origin + IVec3::Y * (height - 5), 0, false),
            trunk_top,
        )];

        for relative_y in (0..=height - 5).rev() {
            let shape = tree_shape(height, relative_y);
            if shape < 0.0 {
                continue;
            }
            for _ in 0..clusters_per_y {
                let radius = 1.0 * shape as f64 * (rng.next_f32() as f64 + 0.328);
                let angle = (rng.next_f32() * 2.0) as f64 * std::f64::consts::PI;
                let x = radius * angle.sin() + 0.5;
                let z = radius * angle.cos() + 0.5;
                let check_start =
                    origin + IVec3::new(x.floor() as i32, relative_y - 1, z.floor() as i32);
                let check_end = check_start + IVec3::Y * 5;
                if !self.make_limb(cx, rng, check_start, check_end, false) {
                    continue;
                }
                let dx = origin.x - check_start.x;
                let dz = origin.z - check_start.z;
                let branch_height =
                    check_start.y as f64 - ((dx * dx + dz * dz) as f64).sqrt() * 0.381;
                let branch_top = if branch_height > trunk_top as f64 {
                    trunk_top
                } else {
                    branch_height as i32
                };
                let check_branch_base = BlockPos::new(origin.x, branch_top, origin.z);
                if self.make_limb(cx, rng, check_branch_base, check_start, false) {
                    foliage_coords
                        .push((FoliageAttachment::new(check_start, 0, false), branch_top));
                }
            }
        }

        self.make_limb(cx, rng, origin, origin + IVec3::Y * trunk_height, true);

        for &(attachment, branch_base) in &foliage_coords {
            let base = BlockPos::new(origin.x, branch_base, origin.z);
            if base != attachment.pos && trim_branches(height, branch_base - origin.y) {
                self.make_limb(cx, rng, base, attachment.pos, true);
            }
        }

        foliage_coords
            .into_iter()
            .filter(|(_, branch_base)| trim_branches(height, branch_base - origin.y))
            .map(|(attachment, _)| attachment)
            .collect()
    }

    fn make_limb<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        start: BlockPos,
        end: BlockPos,
        do_place: bool,
    ) -> bool {
        if !do_place && start == end {
            return true;
        }
        let delta = end - start;
        let steps = delta.abs().max_element();
        let step_x = delta.x as f32 / steps as f32;
        let step_y = delta.y as f32 / steps as f32;
        let step_z = delta.z as f32 / steps as f32;

        for i in 0..=steps {
            let at = start
                + IVec3::new(
                    (0.5 + i as f32 * step_x).floor() as i32,
                    (0.5 + i as f32 * step_y).floor() as i32,
                    (0.5 + i as f32 * step_z).floor() as i32,
                );
            if do_place {
                self.place_log(cx, rng, at, Some(log_axis(start, at)));
            } else if !self.is_free(cx, at) {
                return false;
            }
        }

        true
    }

    fn place_bending<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
        min_height_for_leaves: &Bounded<1, { i32::MAX }, 1>,
        bend_length: &IntProvider,
    ) -> Vec<FoliageAttachment> {
        let direction = random_horizontal(rng);
        let log_height = tree_height - 1;
        let mut at = origin;
        cx.place_below_trunk_block(rng, at - IVec3::Y);
        let mut points = Vec::new();

        for i in 0..=log_height {
            if i + 1 >= log_height + rng.next_i32_bound(2) {
                at += direction.normal();
            }
            if cx.is_valid_tree_pos(at) {
                self.place_log(cx, rng, at, None);
            }
            if i >= min_height_for_leaves.0 {
                points.push(FoliageAttachment::new(at, 0, false));
            }
            at += IVec3::Y;
        }

        let dir_length = bend_length.sample(rng);
        for _ in 0..=dir_length {
            if cx.is_valid_tree_pos(at) {
                self.place_log(cx, rng, at, None);
            }
            points.push(FoliageAttachment::new(at, 0, false));
            at += direction.normal();
        }

        points
    }

    #[allow(clippy::too_many_arguments)]
    fn place_upwards_branching<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
        extra_branch_steps: &IntProvider,
        place_branch_per_log_probability: &UnitFloat,
        extra_branch_length: &IntProvider,
    ) -> Vec<FoliageAttachment> {
        let mut attachments = Vec::new();

        for height_pos in 0..tree_height {
            let current_height = origin.y + height_pos;
            let log_pos = BlockPos::new(origin.x, current_height, origin.z);
            if self.place_log(cx, rng, log_pos, None)
                && height_pos < tree_height - 1
                && rng.next_f32() < place_branch_per_log_probability.0 as f32
            {
                let branch_dir = random_horizontal(rng);
                let branch_len = extra_branch_length.sample(rng);
                let branch_pos = 0.max(branch_len - extra_branch_length.sample(rng) - 1);
                let branch_steps = extra_branch_steps.sample(rng);
                self.place_branch(
                    cx,
                    rng,
                    tree_height,
                    &mut attachments,
                    log_pos,
                    current_height,
                    branch_dir,
                    branch_pos,
                    branch_steps,
                );
            }

            if height_pos == tree_height - 1 {
                attachments.push(FoliageAttachment::new(
                    BlockPos::new(origin.x, current_height + 1, origin.z),
                    0,
                    false,
                ));
            }
        }

        attachments
    }

    #[allow(clippy::too_many_arguments)]
    fn place_branch<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        attachments: &mut Vec<FoliageAttachment>,
        log_pos: BlockPos,
        current_height: i32,
        branch_dir: Direction,
        branch_pos: i32,
        mut branch_steps: i32,
    ) {
        let mut height_along_branch = current_height + branch_pos;
        let mut log_x = log_pos.x;
        let mut log_z = log_pos.z;
        let mut index = branch_pos;

        while index < tree_height && branch_steps > 0 {
            if index >= 1 {
                let placement_height = current_height + index;
                let delta = branch_dir.normal();
                log_x += delta.x;
                log_z += delta.z;
                height_along_branch = placement_height;
                let at = BlockPos::new(log_x, placement_height, log_z);
                if self.place_log(cx, rng, at, None) {
                    height_along_branch = placement_height + 1;
                }
                attachments.push(FoliageAttachment::new(at, 0, false));
            }
            index += 1;
            branch_steps -= 1;
        }

        if height_along_branch - current_height > 1 {
            let foliage_pos = BlockPos::new(log_x, height_along_branch, log_z);
            attachments.push(FoliageAttachment::new(foliage_pos, 0, false));
            attachments.push(FoliageAttachment::new(foliage_pos - IVec3::Y * 2, 0, false));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_cherry<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
        branch_count: &IntProvider,
        branch_start_offset_from_top: &UniformIntRange,
        branch: CherryBranch<'_>,
    ) -> Vec<FoliageAttachment> {
        let (start_min, start_max) = (
            branch_start_offset_from_top.min_inclusive,
            branch_start_offset_from_top.max_inclusive,
        );

        cx.place_below_trunk_block(rng, origin - IVec3::Y);
        let first_offset =
            0.max(tree_height - 1 + rng.next_int_between_inclusive(start_min, start_max));
        let mut second_offset =
            0.max(tree_height - 1 + rng.next_int_between_inclusive(start_min, start_max - 1));
        if second_offset >= first_offset {
            second_offset += 1;
        }

        let count = branch_count.sample(rng);
        let has_middle_branch = count == 3;
        let has_both_side_branches = count >= 2;
        let trunk_height = if has_middle_branch {
            tree_height
        } else if has_both_side_branches {
            first_offset.max(second_offset) + 1
        } else {
            first_offset + 1
        };

        for y in 0..trunk_height {
            self.place_log(cx, rng, origin + IVec3::Y * y, None);
        }

        let mut attachments = Vec::new();
        if has_middle_branch {
            attachments.push(FoliageAttachment::new(
                origin + IVec3::Y * trunk_height,
                0,
                false,
            ));
        }

        let tree_direction = random_horizontal(rng);
        let sideways = tree_direction.axis();
        attachments.push(self.generate_branch(
            cx,
            rng,
            tree_height,
            origin,
            branch,
            sideways,
            tree_direction,
            first_offset,
            first_offset < trunk_height - 1,
        ));
        if has_both_side_branches {
            attachments.push(self.generate_branch(
                cx,
                rng,
                tree_height,
                origin,
                branch,
                sideways,
                tree_direction.opposite(),
                second_offset,
                second_offset < trunk_height - 1,
            ));
        }

        attachments
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_branch<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
        branch: CherryBranch<'_>,
        sideways_axis: Axis,
        branch_direction: Direction,
        offset_from_origin: i32,
        middle_continues_upwards: bool,
    ) -> FoliageAttachment {
        let CherryBranch {
            horizontal_length: branch_horizontal_length,
            end_offset_from_top: branch_end_offset_from_top,
        } = branch;

        let mut log_pos = origin + IVec3::Y * offset_from_origin;
        let branch_end_offset = tree_height - 1 + branch_end_offset_from_top.sample(rng);
        let extend = middle_continues_upwards || branch_end_offset < offset_from_origin;
        let distance_to_trunk = branch_horizontal_length.sample(rng) + i32::from(extend);
        let branch_end_pos =
            origin + branch_direction.normal() * distance_to_trunk + IVec3::Y * branch_end_offset;
        let steps_horizontally = if extend { 2 } else { 1 };

        for _ in 0..steps_horizontally {
            log_pos += branch_direction.normal();
            self.place_log(cx, rng, log_pos, Some(sideways_axis));
        }

        let vertical_direction = if branch_end_pos.y > log_pos.y {
            Direction::Up
        } else {
            Direction::Down
        };

        loop {
            let distance = dist_manhattan(*log_pos, *branch_end_pos);
            if distance == 0 {
                return FoliageAttachment::new(branch_end_pos + IVec3::Y, 0, false);
            }
            let chance = (branch_end_pos.y - log_pos.y).abs() as f32 / distance as f32;
            let grow_vertically = rng.next_f32() < chance;
            log_pos += if grow_vertically {
                vertical_direction
            } else {
                branch_direction
            }
            .normal();
            let axis = if grow_vertically {
                None
            } else {
                Some(sideways_axis)
            };
            self.place_log(cx, rng, log_pos, axis);
        }
    }

    fn place_poplar<W: WorldGenVolume>(
        &self,
        cx: &mut TreeContext<'_, W>,
        rng: &mut XoroshiroRandom,
        tree_height: i32,
        origin: BlockPos,
        trunk_height_above_branches: &IntProvider,
        branch_amount: &IntProvider,
    ) -> Vec<FoliageAttachment> {
        cx.place_below_trunk_block(rng, origin - IVec3::Y);
        let trunk_height_up_to_branches = tree_height - trunk_height_above_branches.sample(rng);

        for y in 0..tree_height {
            self.place_log(cx, rng, origin + IVec3::Y * y, None);
            let directions = shuffled_branch_directions(rng);
            if trunk_height_up_to_branches - 1 == y {
                let branches = branch_amount.sample(rng);
                for index in 0..branches {
                    let branch_direction = directions[index as usize];
                    self.place_log(
                        cx,
                        rng,
                        origin + IVec3::Y * y + branch_direction.normal(),
                        Some(branch_direction.axis()),
                    );
                }
            }
        }

        vec![FoliageAttachment::new(
            origin + IVec3::Y * trunk_height_up_to_branches,
            0,
            false,
        )]
    }
}

/// `Direction.allShuffled` and *then* a filter, so the two vertical faces spend
/// their draws before being thrown away.
fn shuffled_branch_directions(rng: &mut XoroshiroRandom) -> Vec<Direction> {
    all_shuffled(rng)
        .into_iter()
        .filter(|direction| !direction.is_vertical())
        .collect()
}

fn trim_branches(height: i32, local_y: i32) -> bool {
    local_y as f64 >= height as f64 * 0.2
}

fn tree_shape(height: i32, y: i32) -> f32 {
    if (y as f32) < height as f32 * 0.3 {
        return -1.0;
    }
    let radius = height as f32 / 2.0;
    let adjacent = radius - y as f32;
    let mut distance = (radius * radius - adjacent * adjacent).sqrt();
    if adjacent == 0.0 {
        distance = radius;
    } else if adjacent.abs() >= radius {
        return 0.0;
    }
    distance * 0.5
}

fn log_axis(start: BlockPos, at: BlockPos) -> Axis {
    let x_diff = (at.x - start.x).abs();
    let z_diff = (at.z - start.z).abs();
    let max_diff = x_diff.max(z_diff);
    if max_diff == 0 {
        Axis::Y
    } else if x_diff == max_diff {
        Axis::X
    } else {
        Axis::Z
    }
}

#[cfg(test)]
pub(crate) mod harness {
    use rustc_hash::FxHashMap as HashMap;
    use std::sync::Arc;

    use crate::placer::StateMask;
    use fixedbitset::FixedBitSet;
    use mcrs_minecraft_chunk::VoxelId;
    use mcrs_minecraft_random::Random;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::super::provider::StateProvider;
    use super::super::provider::fake::FakeVolume;
    use super::{TreeContext, TreeStates};

    pub const AIR: VoxelId = VoxelId(0);
    pub const LOG: VoxelId = VoxelId(1);
    pub const LOG_X: VoxelId = VoxelId(2);
    pub const LOG_Y: VoxelId = VoxelId(3);
    pub const LOG_Z: VoxelId = VoxelId(4);
    pub const LEAF: VoxelId = VoxelId(5);
    pub const DIRT: VoxelId = VoxelId(6);

    pub fn mask(ids: &[VoxelId]) -> StateMask {
        let mut bits = FixedBitSet::with_capacity(16);
        for id in ids {
            bits.insert(id.0 as usize);
        }
        Arc::new(bits)
    }

    pub fn uniform(min: i32, max: i32) -> String {
        format!(r#"{{"type":"minecraft:uniform","min_inclusive":{min},"max_inclusive":{max}}}"#)
    }

    /// Air is the only free block, `DIRT` is what `can_grow_through` may widen
    /// to, and the log turns into one of three states so an axis rewrite is
    /// visible in the dump.
    pub fn states() -> TreeStates {
        TreeStates {
            valid_tree_pos: mask(&[AIR]),
            logs: mask(&[LOG, LOG_X, LOG_Y, LOG_Z]),
            air_or_leaves: mask(&[AIR, LEAF]),
            persistent: mask(&[]),
            axis: HashMap::from_iter([(LOG.0, [LOG_X, LOG_Y, LOG_Z])]),
            waterlogged: HashMap::default(),
        }
    }

    /// What one placer run is pinned by: every write in order, and the source
    /// it left behind.
    #[derive(Debug, PartialEq, Eq)]
    pub struct Pin {
        pub writes: usize,
        pub digest: u64,
        pub rng_after: [i64; 2],
    }

    fn digest(writes: &[((i32, i32, i32), VoxelId)]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for ((x, y, z), state) in writes {
            for byte in x
                .to_le_bytes()
                .iter()
                .chain(&y.to_le_bytes())
                .chain(&z.to_le_bytes())
                .chain(&state.0.to_le_bytes())
            {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }

    pub fn run<F>(seed: u64, body: F) -> Pin
    where
        F: FnOnce(&mut TreeContext<'_, FakeVolume>, &mut XoroshiroRandom),
    {
        let states = states();
        let trunk = StateProvider::Simple(LOG);
        let foliage = StateProvider::Simple(LEAF);
        let below = StateProvider::Simple(DIRT);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(seed);
        let writes = {
            let mut cx = TreeContext::new(&mut volume, &states, &trunk, &foliage, &below);
            body(&mut cx, &mut rng);
            std::mem::take(&mut cx.volume.writes)
        };
        Pin {
            writes: writes.len(),
            digest: digest(&writes),
            rng_after: [rng.next_i64(), rng.next_i64()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::harness::*;
    use super::*;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    fn placer(kind: &str, rest: &str) -> Trunk {
        placer_through(kind, rest, mask(&[]))
    }

    /// `can_grow_through`, which only `upwards_branching` reads, resolved to
    /// `grow_through`.
    fn placer_through(kind: &str, rest: &str, grow_through: StateMask) -> Trunk {
        let json = format!(r#"{{"type":"minecraft:{kind}_trunk_placer",{rest}}}"#);
        Trunk {
            placer: serde_json::from_str(&json).unwrap(),
            grow_through: Some(grow_through),
        }
    }

    fn base(base_height: i32, height_rand_a: i32, height_rand_b: i32) -> String {
        format!(
            r#""base_height":{base_height},"height_rand_a":{height_rand_a},"height_rand_b":{height_rand_b}"#
        )
    }

    /// One case per trunk type, parameters taken from the shipped feature that
    /// uses it.
    pub(crate) fn cases() -> Vec<(&'static str, Trunk)> {
        vec![
            ("straight", placer("straight", &base(4, 2, 0))),
            ("forking", placer("forking", &base(5, 2, 1))),
            ("giant", placer("giant", &base(10, 2, 15))),
            ("mega_jungle", placer("mega_jungle", &base(10, 2, 15))),
            ("dark_oak", placer("dark_oak", &base(6, 2, 1))),
            ("fancy", placer("fancy", &base(3, 11, 0))),
            (
                "bending",
                placer(
                    "bending",
                    &format!(
                        r#"{},"min_height_for_leaves":4,"bend_length":{}"#,
                        base(5, 2, 1),
                        uniform(1, 4)
                    ),
                ),
            ),
            (
                "upwards_branching",
                placer_through(
                    "upwards_branching",
                    &format!(
                        r#"{},"extra_branch_steps":{},"place_branch_per_log_probability":0.5,"extra_branch_length":{},"can_grow_through":[]"#,
                        base(4, 5, 2),
                        uniform(2, 3),
                        uniform(2, 5)
                    ),
                    mask(&[DIRT]),
                ),
            ),
            (
                "cherry",
                placer(
                    "cherry",
                    &format!(
                        r#"{},"branch_count":{},"branch_horizontal_length":{},"branch_start_offset_from_top":{{"min_inclusive":-4,"max_inclusive":-3}},"branch_end_offset_from_top":{}"#,
                        base(7, 1, 0),
                        uniform(1, 3),
                        uniform(2, 4),
                        uniform(-4, -2)
                    ),
                ),
            ),
            (
                "poplar",
                placer(
                    "poplar",
                    &format!(
                        r#"{},"trunk_height_above_branches":{},"branch_amount":{}"#,
                        base(9, 3, 0),
                        uniform(1, 3),
                        uniform(1, 4)
                    ),
                ),
            ),
        ]
    }

    fn pin_of(placer: &Trunk) -> Pin {
        run(42, |cx, rng| {
            let tree_height = placer.tree_height(rng);
            placer.place_trunk(cx, rng, tree_height, BlockPos::new(8, 64, 8));
        })
    }

    #[test]
    #[ignore = "prints the table the pin test asserts against"]
    fn print_trunk_pins() {
        for (name, placer) in cases() {
            let pin = pin_of(&placer);
            println!(
                "(\"{name}\", {}, {:#x}, {}, {}),",
                pin.writes, pin.digest, pin.rng_after[0], pin.rng_after[1]
            );
        }
    }

    #[test]
    fn every_trunk_placer_writes_and_draws_what_it_did() {
        let expected = TRUNK_PINS;
        assert_eq!(
            cases().len(),
            expected.len(),
            "a trunk placer has no pin row, and zip would skip it"
        );
        for ((name, placer), (pin_name, writes, digest, lo, hi)) in cases().iter().zip(expected) {
            assert_eq!(name, pin_name);
            assert_eq!(
                pin_of(placer),
                Pin {
                    writes: *writes,
                    digest: *digest,
                    rng_after: [*lo, *hi],
                },
                "{name}"
            );
        }
    }

    /// `Util.shuffle` is descending Fisher-Yates: `n - 1` draws for six faces,
    /// a slot as likely to swap with itself as with any other.
    #[test]
    fn all_shuffled_spends_five_draws_over_six_faces() {
        let mut rng = XoroshiroRandom::new(7);
        let shuffled = all_shuffled(&mut rng);

        let mut replay = XoroshiroRandom::new(7);
        for size in (2..=6).rev() {
            replay.next_i32_bound(size);
        }
        assert_eq!(rng.next_i64(), replay.next_i64());

        let mut sorted = shuffled;
        sorted.sort_by_key(|direction| *direction as u8);
        assert_eq!(sorted, Direction::all());
    }

    /// `Direction.allShuffled` is filtered *after* the shuffle, so a placer that
    /// wants horizontals still pays for the two vertical faces.
    #[test]
    fn poplar_branch_directions_burn_the_vertical_draws() {
        let mut rng = XoroshiroRandom::new(3);
        let directions = shuffled_branch_directions(&mut rng);
        assert_eq!(directions.len(), 4);

        let mut replay = XoroshiroRandom::new(3);
        assert_eq!(all_shuffled(&mut replay).len(), 6);
        assert_eq!(rng.next_i64(), replay.next_i64());
    }

    /// `base_height + nextInt(a + 1) + nextInt(b + 1)`, in that order.
    #[test]
    fn tree_height_draws_a_then_b() {
        let placer = placer("straight", &base(4, 2, 3));
        let mut rng = XoroshiroRandom::new(11);
        let height = placer.tree_height(&mut rng);

        let mut replay = XoroshiroRandom::new(11);
        let expected = 4 + replay.next_i32_bound(3) + replay.next_i32_bound(4);
        assert_eq!(height, expected);
        assert_eq!(rng.next_i64(), replay.next_i64());
    }

    /// `upwards_branching` widens `validTreePos`, and `isFree` is that widened
    /// test or a log — so a block only `can_grow_through` names is free.
    #[test]
    fn upwards_branching_grows_through_its_own_set() {
        let states = states();
        let trunk = StateProvider::Simple(LOG);
        let mut volume = super::super::provider::fake::FakeVolume::with([((0, 0, 0), DIRT)]);
        let cx = TreeContext::new(&mut volume, &states, &trunk, &trunk, &trunk);

        let branching = format!(
            r#"{},"extra_branch_steps":1,"place_branch_per_log_probability":0.5,"extra_branch_length":1,"can_grow_through":[]"#,
            base(4, 0, 0)
        );
        let widened = placer_through("upwards_branching", &branching, mask(&[DIRT]));
        let plain = placer("upwards_branching", &branching);
        let at = BlockPos::new(0, 0, 0);

        assert!(widened.valid_tree_pos(&cx, at));
        assert!(!plain.valid_tree_pos(&cx, at));
        assert!(!cx.is_free(at));
        assert!(widened.is_free(&cx, at));
    }

    const TRUNK_PINS: &[(&str, usize, u64, i64, i64)] = &[
        (
            "straight",
            6,
            0x5bd5b5920ba70571,
            -7542733514721318211,
            4888889476139319686,
        ),
        (
            "forking",
            9,
            0x322993003ba0a4be,
            -9216016236493209689,
            -8459460178172966819,
        ),
        (
            "giant",
            65,
            0xce21451beb34137b,
            -7542733514721318211,
            4888889476139319686,
        ),
        (
            "mega_jungle",
            69,
            0xd96691a9ea8d81d1,
            -6491934549477179079,
            8279452174680803839,
        ),
        (
            "dark_oak",
            40,
            0x5adbd297cd15f0e5,
            -3782256110256217100,
            8111380311640346563,
        ),
        (
            "fancy",
            9,
            0xe2762a599351c746,
            8279452174680803839,
            6246239634032559210,
        ),
        (
            "bending",
            12,
            0x4516a4d1bc0d361d,
            9206045781291848097,
            9081162010021283615,
        ),
        (
            "upwards_branching",
            9,
            0x60e3bab9d7952246,
            6291911111440803531,
            -3066404182989085885,
        ),
        (
            "cherry",
            16,
            0x20e0c269ea15e7e3,
            -1441244676496472421,
            8296740669997502935,
        ),
        (
            "poplar",
            15,
            0x7da769804674a1dd,
            -6172206221005592769,
            -8132412617383598691,
        ),
    ];
}
