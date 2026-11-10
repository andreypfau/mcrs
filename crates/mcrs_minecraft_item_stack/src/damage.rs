/// The damage that the stack has taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Damage(pub i32);

impl Damage {
    /// Gives `None` for a value that the saved form does not hold: one out of `0..=i32::MAX`.
    #[must_use]
    pub const fn new(value: i32) -> Option<Self> {
        if value >= 0 { Some(Self(value)) } else { None }
    }
}
