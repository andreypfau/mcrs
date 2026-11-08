/// A packet that a mob sends its viewers once, besides its state (DMG-NET-001, DMG-NET-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobSignal {
    /// A full hit, with the network id of its damage type.
    Hurt { damage_type: u32 },
    /// The death: the entity event 3.
    Died,
    /// The removal after the death: the entity event 60.
    Removed,
}
