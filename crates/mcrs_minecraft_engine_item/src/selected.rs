use bevy_ecs::component::Component;

/// The selected slot of a holder: an index into its [`Slots`](crate::Slots).
///
/// An index without a slot, or with an empty slot, selects no stack.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selected(pub u8);
