bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct BlockFlags: u32 {
        const FLUID = 1 << 0;
        const CLIMBABLE = 1 << 1;
        const FENCE = 1 << 2;
        const WALL = 1 << 3;
        const FENCE_GATE = 1 << 4;
        const SUPPRESSES_BOUNCE = 1 << 5;
        const FALL_DAMAGE_RESETTING = 1 << 6;
        const CAN_GLIDE_THROUGH = 1 << 7;
        const LARGE = 1 << 8;
        const MOVING_PISTON = 1 << 9;
        const OWN_SPEED_FACTOR = 1 << 10;
        const SLIME = 1 << 11;
        const STEP_ACTION = 1 << 12;
        const INSIDE_ACTION = 1 << 13;
        const OFF_GRID = 1 << 14;
        const COARSE_NEAR = 1 << 15;
        const COARSE_BUBBLE_COLUMN = 1 << 16;
        const COARSE_POTENT_SULFUR = 1 << 17;
        const COARSE_PISTON = 1 << 18;
        const SUFFOCATES = 1 << 19;
        const SOLID_RENDER = 1 << 20;
        const LEGACY_SOLID = 1 << 21;
        const WATER = 1 << 22;
        const GRASS_BLOCK = 1 << 23;
        const DOOR = 1 << 24;
        const LIQUID_BLOCK = 1 << 25;
    }
}
