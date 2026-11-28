use bevy_ecs::system::Commands;
use glam::DVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;
use mcrs_minecraft_vanilla_item_entity::Launch;
use mcrs_voxel::BlockStateId;

use crate::falling_block_rules::FallingBlockRules;

pub(crate) fn drop_item(
    mut commands: Commands<'_, '_>,
    rules: &FallingBlockRules,
    definitions: &BlockDefinitions,
    carried: BlockStateId,
    position: DVec3,
    key: EntityUuid,
) {
    let (Some(kind), Some(count)) = (rules.item_of(definitions, carried), Count::new(1)) else {
        return;
    };
    commands
        .spawn_stack(ItemStack::new(kind, count))
        .launch(Launch {
            source: BirthSource::Entity(key),
            form: Form::Default(position),
            values: ItemEntity {
                pickup_delay: Cause::BlockDrop.pickup_delay(),
                ..ItemEntity::default()
            },
        });
}
