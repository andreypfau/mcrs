/// The stack cannot take damage. Only the presence of the component has a meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Unbreakable;
