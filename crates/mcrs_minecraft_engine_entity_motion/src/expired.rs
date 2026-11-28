use core::fmt;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_utils::Parallel;
use mcrs_minecraft_entity_key::EntityUuid;

use crate::ExpiryCause;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expiry {
    pub entity: Entity,
    pub key: EntityUuid,
    pub cause: ExpiryCause,
}

#[derive(Resource, Default)]
pub struct Expired {
    pub(crate) queues: Parallel<Vec<Expiry>>,
    sorted: Vec<Expiry>,
}

impl Expired {
    #[must_use]
    pub fn entries(&self) -> &[Expiry] {
        &self.sorted
    }

    pub(crate) fn collect(&mut self) {
        let Self { queues, sorted } = self;
        sorted.clear();
        queues.drain_into(sorted);
        sorted.sort_unstable_by_key(|expiry| expiry.key);
    }
}

impl fmt::Debug for Expired {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Expired")
            .field("sorted", &self.sorted)
            .finish_non_exhaustive()
    }
}
