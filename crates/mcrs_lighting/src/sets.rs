//! `LightingSet` covers the per-tick lighting pipeline. `Enqueue` /
//! `Converge` / `EmitDirty` run in `FixedUpdate`; `Codec` runs strictly after
//! them in `FixedPostUpdate` so the wire codec reads the converged storage
//! without racing the propagate systems' `&mut BlockLight` / `&mut SkyLight`
//! writes.

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
    Codec,
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
        let _ = LightingSet::Codec;
    }
}
