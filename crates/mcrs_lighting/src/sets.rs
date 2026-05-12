//! Intra-section subset of `LightingSet`. Later work extends this enum with
//! convergence-loop, emit-dirty, and codec variants chained in
//! `FixedPostUpdate`. The intra-section variants are deliberately not
//! pre-declared alongside the convergence variants to avoid dead-code lints.

use bevy_ecs::schedule::SystemSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LightingSet {
    Enqueue,
    PropagateDecrease,
    PropagateIncrease,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_set_variants_compile() {
        let _ = LightingSet::Enqueue;
        let _ = LightingSet::PropagateDecrease;
        let _ = LightingSet::PropagateIncrease;
    }
}
