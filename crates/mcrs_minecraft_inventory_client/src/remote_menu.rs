use bevy_ecs::component::Component;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteMenu {
    container_id: i32,
    state_id: i32,
}

impl RemoteMenu {
    pub(crate) const INVENTORY: i32 = 0;

    pub(crate) fn inventory(state_id: i32) -> Self {
        Self {
            container_id: Self::INVENTORY,
            state_id,
        }
    }

    #[must_use]
    pub fn container_id(&self) -> i32 {
        self.container_id
    }

    #[must_use]
    pub fn state_id(&self) -> i32 {
        self.state_id
    }
}
