/// One active effect of a living entity, as a mob effects predicate reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveEffect {
    pub amplifier: i32,
    /// The ticks left, or −1 for an effect with no end.
    pub duration: i32,
    pub ambient: bool,
    pub visible: bool,
}
