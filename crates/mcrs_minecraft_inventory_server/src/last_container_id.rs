use bevy_ecs::component::Component;

const MAX_CONTAINER_ID: u8 = 100;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LastContainerId(u8);

impl LastContainerId {
    pub(crate) fn advance(&mut self) -> u8 {
        let next = if self.0 >= MAX_CONTAINER_ID {
            1
        } else {
            self.0.saturating_add(1)
        };
        self.0 = next;
        next
    }
}
