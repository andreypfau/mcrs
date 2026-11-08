use crate::Body;

/// Whether the physics part of a tick can be skipped. A skip is exact: it gives the state of the
/// full tick, because a fixed point stays one while nothing in the read zone changes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Rest {
    #[default]
    Awake,
    /// The state after the last tick of the cycle; the next such tick gives it again.
    Candidate(Body),
    /// The state of the fixed point. An item keeps the state after the tick of its cycle in which
    /// it moves.
    Resting(Body),
}

/// True when the two states are equal bit for bit.
#[must_use]
pub fn same_bits(a: &Body, b: &Body) -> bool {
    let bits = |body: &Body| {
        [body.position, body.velocity, body.stuck].map(|vector| vector.to_array().map(f64::to_bits))
    };
    bits(a) == bits(b)
        && a.fall_distance.to_bits() == b.fall_distance.to_bits()
        && a.flags == b.flags
        && a.support == b.support
        && a.support_missed == b.support_missed
}
