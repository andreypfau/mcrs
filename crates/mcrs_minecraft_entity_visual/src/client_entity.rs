use bevy_ecs::component::Component;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::{ClientMotion, EntityTypeData};
use uuid::Uuid;

/// An entity that the server added to the client, with the id that its packets name.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientEntity {
    pub network_id: i32,
    pub uuid: Uuid,
    pub kind: EntityType,
}

impl ClientEntity {
    /// The type data of the kind, or `None` for a kind that the table does not list yet.
    #[must_use]
    pub fn type_data(&self) -> Option<&'static EntityTypeData> {
        EntityTypeData::of(self.kind)
    }

    /// The ticks between two position updates of the server.
    #[must_use]
    pub fn update_interval(&self) -> u32 {
        self.type_data()
            .map_or(EntityTypeData::DEFAULT_UPDATE_INTERVAL, |data| {
                data.update_interval
            })
    }

    /// Whether the client moves the entity between two updates of the server with the step of
    /// its type, as the type data states.
    #[must_use]
    pub fn is_simulated(&self) -> bool {
        self.type_data()
            .is_some_and(|data| matches!(data.client_motion, ClientMotion::Simulated(_)))
    }
}
