/// The damage value at which the stack breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct MaxDamage(pub i32);

impl MaxDamage {
    /// Gives `None` for a value that the saved form does not hold: one out of `1..=i32::MAX`.
    #[must_use]
    pub const fn new(value: i32) -> Option<Self> {
        if value >= 1 { Some(Self(value)) } else { None }
    }
}
