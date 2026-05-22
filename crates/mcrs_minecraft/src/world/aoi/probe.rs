//! Verification probe Resource for the stationary-zero-work invariant.
//! The AoI system bodies (`aoi::update_own_pov`,
//! `aoi::update_tracked_by`) increment the matching counter on entry;
//! the stationary-zero-work integration test snapshots the counters
//! after the first tick and asserts they stay flat across subsequent
//! stationary ticks.

use bevy_ecs::resource::Resource;

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AoiTickProbe {
    pub own_pov_ran: u32,
    pub tracked_by_ran: u32,
}
