use bevy_ecs::component::Component;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
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
}
