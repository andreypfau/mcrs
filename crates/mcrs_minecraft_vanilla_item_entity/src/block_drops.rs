use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Commands;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_world::block_drops::ItemDropped;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;

use crate::Form;
use crate::Launch;
use crate::item_entity_commands_ext::ItemEntityCommandsExt;

/// Makes each item that a block left an item entity, as the row "drop of a block" of IEN-SPN-001
/// states.
pub(crate) fn launch_block_drops(mut dropped: MessageReader<ItemDropped>, mut commands: Commands) {
    for drop in dropped.read() {
        let Some(count) = Count::new(drop.stack.count) else {
            continue;
        };
        let stack = ItemStack::new(Kind(drop.stack.item.number()), count);
        commands.spawn_stack(stack).launch(Launch {
            source: BirthSource::Block(drop.at),
            form: Form::Block(drop.at.as_dvec3()),
            values: ItemEntity {
                pickup_delay: Cause::BlockDrop.pickup_delay(),
                ..ItemEntity::default()
            },
        });
    }
}
