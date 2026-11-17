#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct MaxStackSize(pub i32);

impl MaxStackSize {
    #[must_use]
    pub const fn new(value: i32) -> Option<Self> {
        if value >= 1 && value <= 99 {
            Some(Self(value))
        } else {
            None
        }
    }
}
