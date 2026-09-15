//! Verification probe Resource for the stationary-zero-work invariant.
//! `aoi::update_tracked_by` increments the counter on entry; the
//! stationary-zero-work integration test snapshots it after the first tick
//! and asserts it stays flat across subsequent stationary ticks.

use bevy_ecs::resource::Resource;

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AoiTickProbe {
    pub tracked_by_ran: u32,
}
