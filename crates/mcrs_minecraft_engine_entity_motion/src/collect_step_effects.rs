use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::SystemChangeTick;

use crate::CrowdCandidates;
use crate::Expired;
use crate::regrids::Regrids;
use crate::regrids::stepped_since;

pub(crate) fn step_effects_pending(
    crowds: Res<CrowdCandidates>,
    expired: Res<Expired>,
    regrids: Res<Regrids>,
    ticks: SystemChangeTick,
) -> bool {
    !crowds.entries().is_empty() || !expired.entries().is_empty() || stepped_since(&regrids, &ticks)
}

pub(crate) fn collect_step_effects(
    mut crowds: ResMut<CrowdCandidates>,
    mut expired: ResMut<Expired>,
) {
    crowds.collect();
    expired.collect();
}
