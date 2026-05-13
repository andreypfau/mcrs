//! Workspace anchor for the `LightTicket` sparse-marker component.
//!
//! This marker lives in `mcrs_engine` (rather than alongside the other
//! lighting components in `mcrs_lighting::components`) because the lighting
//! crate already declares `mcrs_minecraft.workspace = true`. Hosting the
//! marker in `mcrs_lighting` would force a `mcrs_minecraft -> mcrs_lighting`
//! dependency for the chunk-cancellation guard, completing a workspace cycle
//! that cargo refuses to resolve. Both downstream crates depend on
//! `mcrs_engine`, so this module is the only valid shared anchor for the
//! marker. `mcrs_engine` carries no other lighting knowledge.
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
