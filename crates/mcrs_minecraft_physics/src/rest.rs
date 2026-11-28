use crate::Body;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Rest {
    #[default]
    Awake,
    Candidate(Body),
    Resting(Body),
}

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
