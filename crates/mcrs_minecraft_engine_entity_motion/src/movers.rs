use core::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use bevy_utils::Parallel;

/// The entities whose motion changed in the last [`MotionPass`](crate::MotionPass) in a way that a
/// viewer sees: a new position, new collision flags, a bounce, a new velocity after a wake, or a
/// write of another system since the last pass. A change of the velocity of a resting entity alone
/// is not listed. The pass replaces the list in
/// each tick, so it holds from the end of one pass to the start of the next. The entities are
/// sorted and appear once.
#[derive(Resource, Default)]
pub struct Movers {
    pub(crate) queues: Parallel<Vec<Entity>>,
    pub(crate) sorted: Vec<Entity>,
}

impl Movers {
    /// The entities that moved in the last pass, sorted, each once.
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.sorted
    }
}

/// Replaces the list with what the step queued; it runs even when the step does not, so that the
/// list of an earlier tick does not stay.
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
