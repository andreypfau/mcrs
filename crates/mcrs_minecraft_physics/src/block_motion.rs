/// The four motion properties of a block, as f32 values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockMotion {
    pub friction: f32,
    pub speed_factor: f32,
    pub jump_factor: f32,
    pub bounce: f32,
}

impl BlockMotion {
    /// The values of a block that states none, and of an empty cell.
    pub const DEFAULT: BlockMotion = BlockMotion {
        friction: 0.6,
        speed_factor: 1.0,
        jump_factor: 1.0,
        bounce: 0.0,
    };
}

impl Default for BlockMotion {
    fn default() -> Self {
        Self::DEFAULT
    }
}
