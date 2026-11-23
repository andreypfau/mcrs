use bevy_ecs::system::ResMut;

use crate::CrowdCandidates;
use crate::Expired;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "clears the crowd candidates and the expiries of the previous tick, which must not survive a tick without steps"
    )
)]
pub(crate) fn collect_step_effects(
    mut crowds: ResMut<CrowdCandidates>,
    mut expired: ResMut<Expired>,
) {
    crowds.collect();
    expired.collect();
}
