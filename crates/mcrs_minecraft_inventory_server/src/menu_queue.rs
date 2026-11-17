use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

#[derive(Resource, Debug, Default)]
pub(crate) struct MenuQueue {
    pub(crate) content: Vec<Entity>,
}

impl MenuQueue {
    pub(crate) fn settle(&mut self) {
        self.content.sort_unstable();
        self.content.dedup();
    }
}
