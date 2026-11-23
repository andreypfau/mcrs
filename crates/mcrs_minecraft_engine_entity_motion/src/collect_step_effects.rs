use bevy_ecs::system::ResMut;

use crate::CrowdCandidates;
use crate::Expired;

pub(crate) fn collect_step_effects(
    mut crowds: ResMut<CrowdCandidates>,
    mut expired: ResMut<Expired>,
) {
    crowds.collect();
    expired.collect();
}
