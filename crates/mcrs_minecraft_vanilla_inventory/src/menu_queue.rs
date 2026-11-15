use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

/// The players whose inventory menu the next sync sends in full instead of by comparison with
/// the client view (INV-SYN-005, ITX-JAV-004).
#[derive(Resource, Debug, Default)]
pub(crate) struct MenuQueue {
    pub(crate) content: Vec<Entity>,
}

impl MenuQueue {
    /// Sorts the list and removes repeats.
    pub(crate) fn settle(&mut self) {
        self.content.sort_unstable();
        self.content.dedup();
    }
}
