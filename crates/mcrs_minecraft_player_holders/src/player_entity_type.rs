use alloc::vec::Vec;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::lifecycle::Insert;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::FromWorld;
use bevy_ecs::world::World;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_item_stack::EntityTypeId;
use mcrs_minecraft_item_stack::EntityTypeSet;
use mcrs_minecraft_item_stack::EntityTypeTagId;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Registered;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlayerEntityType {
    id: Option<EntityTypeId>,
    tags: Vec<EntityTypeTagId>,
}

impl PlayerEntityType {
    pub(crate) const UNKNOWN: Self = Self {
        id: None,
        tags: Vec::new(),
    };

    fn of(registries: &Registries) -> Self {
        let registry = EntityType::REGISTRY.location();
        let Some(player) = registries.id_of(registry, EntityType::Player.location()) else {
            return Self::UNKNOWN;
        };
        let mut tags = Vec::new();
        if let Some(entity_type_tags) = registries.tags::<EntityType>() {
            for (tag, _) in entity_type_tags.iter() {
                let mut members = entity_type_tags.members(tag);
                if members.any(|member| member.number() == player) {
                    tags.push(EntityTypeTagId(tag.number()));
                }
            }
        }
        Self {
            id: Some(EntityTypeId(player)),
            tags,
        }
    }

    pub(crate) fn admitted_by(&self, set: Option<&EntityTypeSet>) -> bool {
        match set {
            None => true,
            Some(EntityTypeSet::EntityTypes(entity_types)) => {
                self.id.is_some_and(|id| entity_types.contains(&id))
            }
            Some(EntityTypeSet::Tag(tag)) => self.tags.contains(tag),
        }
    }
}

impl FromWorld for PlayerEntityType {
    fn from_world(world: &mut World) -> Self {
        world
            .get_resource::<CurrentRegistries>()
            .map(|registries| Self::of(registries))
            .unwrap_or(Self::UNKNOWN)
    }
}

pub(crate) fn derive_player_entity_type(
    _: On<Insert<CurrentRegistries>>,
    registries: Res<CurrentRegistries>,
    mut player: ResMut<PlayerEntityType>,
) {
    player.set_if_neq(PlayerEntityType::of(&registries));
}

pub(crate) fn forget_player_entity_type(
    _: On<Remove<CurrentRegistries>>,
    mut player: ResMut<PlayerEntityType>,
) {
    player.set_if_neq(PlayerEntityType::UNKNOWN);
}
