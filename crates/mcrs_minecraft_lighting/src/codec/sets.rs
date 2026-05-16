//! `LightingSet` covers the per-tick lighting pipeline. `Enqueue` /
//! `Converge` / `EmitDirty` run in `FixedUpdate`; `Codec` runs strictly after
//! them in `FixedPostUpdate` so the wire codec reads the converged storage
//! without racing the propagate systems' `&mut BlockLight` / `&mut SkyLight`
//! writes.

use bevy_ecs::schedule::SystemSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LightingSet {
    Enqueue,
    Converge,
    EmitDirty,
    Codec,
}

#[cfg(test)]
mod tests {
    use super::*;
}
