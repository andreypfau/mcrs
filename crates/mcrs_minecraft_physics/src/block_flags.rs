bitflags::bitflags! {
    /// What the physics reads about a block state besides its shape.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct BlockFlags: u32 {
        /// The state holds a fluid: the fluid model applies.
        const FLUID = 1 << 0;
        /// The block tag `climbable`.
        const CLIMBABLE = 1 << 1;
        /// The block tag `fences`.
        const FENCE = 1 << 2;
        /// The block tag `walls`.
        const WALL = 1 << 3;
        /// The block tag `fence_gates`.
        const FENCE_GATE = 1 << 4;
        /// The block tag `suppresses_bounce`.
        const SUPPRESSES_BOUNCE = 1 << 5;
        /// The block tag `fall_damage_resetting`.
        const FALL_DAMAGE_RESETTING = 1 << 6;
        /// The block tag `can_glide_through`.
        const CAN_GLIDE_THROUGH = 1 << 7;
        /// The shape for no context goes out of the unit cube: the block scan reads it on the
        /// outer faces of its region.
        const LARGE = 1 << 8;
        /// A `moving_piston`: the block scan reads it on the outer edges of its region.
        const MOVING_PISTON = 1 << 9;
        /// The speed factor of the block applies even when it is 1.0: water and bubble columns.
        const OWN_SPEED_FACTOR = 1 << 10;
        /// The step action of a slime block.
        const SLIME = 1 << 11;
        /// The block has a step action, which the tick must run on the block domain.
        const STEP_ACTION = 1 << 12;
        /// The block has an inside action, which the tick must run on the block domain.
        const INSIDE_ACTION = 1 << 13;
        /// A coordinate of the shape is off the grid of 1/16: the corpus rounds it.
        const OFF_GRID = 1 << 14;
        /// A block in the near zone makes the tick coarse: a fluid, a portal, a shulker box, a
        /// big dripleaf, a shape with a position offset, or an effect the model does not state.
        const COARSE_NEAR = 1 << 15;
        /// A bubble column, which acts up to 2 blocks above it.
        const COARSE_BUBBLE_COLUMN = 1 << 16;
        /// Potent sulfur, whose geyser acts up to 25 blocks above it.
        const COARSE_POTENT_SULFUR = 1 << 17;
        /// A piston, a piston head or a moving piston, which acts up to 2 blocks away.
        const COARSE_PISTON = 1 << 18;
    }
}
