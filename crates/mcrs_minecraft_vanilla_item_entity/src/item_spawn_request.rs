use bevy_ecs::component::Component;
use bevy_ecs::message::Message;
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Commands;
use glam::DVec3;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_item_stack::ItemStack;

use crate::Form;
use crate::ItemEntityCommandsExt;
use crate::Launch;
use crate::Owner;
use crate::Thrower;

/// An item entity at rest at `position` with the values of a saved form and the UUID `key`, as
/// the `summon item` command makes it (IEN-SPN-001).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ItemSpawnRequest {
    pub stack: ItemStack,
    pub position: DVec3,
    pub key: EntityKey,
    pub values: ItemEntity,
    pub owner: Option<u128>,
    pub thrower: Option<u128>,
}

/// The UUID that a request gave the item entity, in place of the one of its birth.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct FixedKey(pub(crate) EntityKey);

pub(crate) fn spawn_requested(
    mut requests: MessageReader<ItemSpawnRequest>,
    mut commands: Commands,
) {
    for request in requests.read() {
        let mut stack = commands.spawn_stack(request.stack.clone());
        stack
            .launch(Launch {
                source: BirthSource::Entity(request.key),
                form: Form::Fixed(Start {
                    position: request.position,
                    velocity: DVec3::ZERO,
                }),
                values: request.values,
            })
            .insert(FixedKey(request.key));
        if let Some(owner) = request.owner {
            stack.insert(Owner(owner));
        }
        if let Some(thrower) = request.thrower {
            stack.insert(Thrower(thrower));
        }
    }
}
