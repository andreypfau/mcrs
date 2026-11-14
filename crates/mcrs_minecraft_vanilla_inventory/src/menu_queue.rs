use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;

/// The players whose inventory menu the next sync sends: in full, or by comparison with the
/// client view (INV-SYN-003, INV-SYN-005).
// chisle: a comparison follows a click, a request result and a give. A write of a holder by
// another path (`put_at`, `put_in`, `swap_with`, `consume`, `drop_at`, a component write, a
// despawn) reaches the client only at the next of those; an engine message for each slot write
// lifts it.
#[derive(Resource, Debug, Default)]
pub(crate) struct MenuQueue {
    pub(crate) content: Vec<Entity>,
    pub(crate) compare: Vec<Entity>,
}

impl MenuQueue {
    /// Sorts each list and removes repeats and each player of `compare` that `content` holds.
    pub(crate) fn settle(&mut self) {
        self.content.sort_unstable();
        self.content.dedup();
        self.compare.sort_unstable();
        self.compare.dedup();
        let content = &self.content;
        self.compare
            .retain(|player| content.binary_search(player).is_err());
    }
}

pub(crate) fn any_queued(queue: Res<MenuQueue>) -> bool {
    !queue.content.is_empty() || !queue.compare.is_empty()
}
