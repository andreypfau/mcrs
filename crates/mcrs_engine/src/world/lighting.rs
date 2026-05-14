//! Workspace anchor for the `LightTicket` sparse-marker component.
//!
//! This marker lives in `mcrs_engine` (rather than alongside the other
//! lighting components in `mcrs_minecraft_lighting::components`) because both
//! `mcrs_minecraft` and `mcrs_minecraft_lighting` need to reference it from
//! upstream paths. Hosting the marker in `mcrs_minecraft_lighting` would force
//! a `mcrs_minecraft -> mcrs_minecraft_lighting` dependency for the
//! chunk-cancellation guard, which is exactly what the workspace split avoids.
//! Both downstream crates depend on `mcrs_engine`, so this module is the only
//! valid shared anchor for the marker. `mcrs_engine` carries no other
//! lighting knowledge.
use bevy_ecs::prelude::Component;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct LightTicket;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_ticket_marker_compile_test() {
        let _m = LightTicket;
    }
}
