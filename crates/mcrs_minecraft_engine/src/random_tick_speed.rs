use bevy_ecs::resource::Resource;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomTickSpeed(pub u32);

impl Default for RandomTickSpeed {
    fn default() -> Self {
        Self(3)
    }
}
