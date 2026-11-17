use bevy_ecs::component::Component;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::{ClientMotion, EntityTypeData};
use uuid::Uuid;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientEntity {
    pub network_id: i32,
    pub uuid: Uuid,
    pub kind: EntityType,
}

impl ClientEntity {
    #[must_use]
    pub fn type_data(&self) -> Option<&'static EntityTypeData> {
        EntityTypeData::of(self.kind)
    }

    #[must_use]
    pub fn update_interval(&self) -> u32 {
        self.type_data()
            .map_or(EntityTypeData::DEFAULT_UPDATE_INTERVAL, |data| {
                data.update_interval
            })
    }

    #[must_use]
    pub fn is_simulated(&self) -> bool {
        self.type_data()
            .is_some_and(|data| matches!(data.client_motion, ClientMotion::Simulated(_)))
    }
}
