use crate::damage_source::DamageSource;

/// The damage event that a damage predicate tests. `E` is the entity handle of the
/// [`crate::EntityView`] of the match.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageEvent<E> {
    /// The player of the event; its position is the origin of the nested entity predicates.
    pub player: E,
    /// The source of the damage that the damage source predicate `type` tests.
    pub source: DamageSource<E>,
    /// The amount before the reductions.
    pub dealt: f64,
    /// The amount after the reductions.
    pub taken: f64,
    /// `true` when a shield blocked the damage.
    pub blocked: bool,
}
