use core::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_utils::Parallel;
use mcrs_minecraft_physics::GridRow;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrowdCandidate {
    pub entity: Entity,
    pub own: GridRow,
    pub candidates: u32,
    pub free: u32,
}

#[derive(Resource, Default)]
pub struct CrowdCandidates {
    pub(crate) queues: Parallel<Vec<CrowdCandidate>>,
    sorted: Vec<CrowdCandidate>,
}

impl CrowdCandidates {
    #[must_use]
    pub fn entries(&self) -> &[CrowdCandidate] {
        &self.sorted
    }

    pub(crate) fn collect(&mut self) {
        let Self { queues, sorted } = self;
        sorted.clear();
        queues.drain_into(sorted);
        sorted.sort_unstable_by_key(|candidate| candidate.own.key);
    }
}

impl fmt::Debug for CrowdCandidates {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CrowdCandidates")
            .field("sorted", &self.sorted)
            .finish_non_exhaustive()
    }
}
