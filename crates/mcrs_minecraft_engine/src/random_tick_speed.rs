use bevy_ecs::resource::Resource;

/// The game rule `minecraft:random_tick_speed`: the random picks in each section that takes
/// random ticks, in each tick of a dimension. The host holds it, and each dimension world takes
/// a copy when it spawns.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomTickSpeed(pub u32);

impl Default for RandomTickSpeed {
    fn default() -> Self {
        Self(3)
    }
}
