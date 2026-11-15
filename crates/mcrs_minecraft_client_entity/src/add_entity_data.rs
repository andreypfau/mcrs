use bevy_ecs::component::Component;

/// The data value of the packet that added a falling block: the numeric id of its block state.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddEntityData(pub i32);
