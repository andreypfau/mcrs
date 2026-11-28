#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Damage(pub i32);

impl Damage {
    #[must_use]
    pub const fn new(value: i32) -> Option<Self> {
        if value >= 0 { Some(Self(value)) } else { None }
    }
}
