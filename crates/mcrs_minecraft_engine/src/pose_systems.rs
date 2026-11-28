use bevy_ecs::schedule::SystemSet;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PoseSystems {
    Select,
    Publish,
}
