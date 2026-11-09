/// The hits on a living entity that its AI reads (DMG-HIT-002 step 1, `mob-goals.md`
/// GOAL-PNC-001 and GOAL-STR-004). The damage procedure writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HitRecord {
    /// The last hit that the damage cooldown accepted.
    pub last: Option<LastHit>,
    /// The count of the hits that passed the tests of DMG-HIT-001, also those that the cooldown
    /// rejected. Each one resets the idle time; the count wraps.
    pub strikes: u32,
}

/// An accepted hit: its tick and the network id of its damage type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastHit {
    pub tick: u64,
    pub damage_type: u32,
}
