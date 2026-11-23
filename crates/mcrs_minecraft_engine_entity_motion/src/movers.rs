use core::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use bevy_utils::Parallel;

#[derive(Resource, Default)]
pub struct Movers {
    pub(crate) queues: Parallel<Vec<Entity>>,
    pub(crate) sorted: Vec<Entity>,
}

impl Movers {
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.sorted
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "clears the movers of the previous tick, which must not survive a tick without movers"
    )
)]
pub(crate) fn collect_movers(mut movers: ResMut<Movers>) {
    let Movers { queues, sorted } = &mut *movers;
    sorted.clear();
    queues.drain_into(sorted);
    sorted.sort_unstable();
}

impl fmt::Debug for Movers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Movers")
            .field("sorted", &self.sorted)
            .finish_non_exhaustive()
    }
}
