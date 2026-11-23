use core::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::SystemChangeTick;
use bevy_utils::Parallel;

use crate::regrids::Regrids;
use crate::regrids::stepped_since;

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

pub(crate) fn movers_pending(
    movers: Res<Movers>,
    regrids: Res<Regrids>,
    ticks: SystemChangeTick,
) -> bool {
    !movers.sorted.is_empty() || stepped_since(&regrids, &ticks)
}

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
