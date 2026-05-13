//! Intra-section subset of `LightingSet`. Later work extends this enum with
//! convergence-loop, emit-dirty, and codec variants chained in
//! `FixedPostUpdate`. The intra-section variants are deliberately not
//! pre-declared alongside the convergence variants to avoid dead-code lints.

use bevy_ecs::schedule::SystemSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LightingSet {
    Enqueue,
    /// Legacy intra-section variant retained for symbol stability. No longer
    /// referenced by the production schedule; the per-section propagate
    /// systems run inside `LightConvergeSchedule::PropagateDecrease`.
    #[allow(dead_code)]
    PropagateDecrease,
    /// Legacy intra-section variant retained for symbol stability. No longer
    /// referenced by the production schedule; the per-section propagate
    /// systems run inside `LightConvergeSchedule::PropagateIncrease`.
    #[allow(dead_code)]
    PropagateIncrease,
    Converge,
    EmitDirty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_set_variants_compile() {
        let _ = LightingSet::Enqueue;
        let _ = LightingSet::PropagateDecrease;
        let _ = LightingSet::PropagateIncrease;
        let _ = LightingSet::Converge;
        let _ = LightingSet::EmitDirty;
    }
}
