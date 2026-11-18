use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_inventory_menu::worn_slot;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::Offer;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_item_stack::Equippable;

use crate::player_entity_type::PlayerEntityType;

pub(crate) fn armour_place(world: &World, query: &GateQuery) -> Filter {
    let Some(rule) = Holder::Inventory.slot(query.slot.index) else {
        return Filter::Refuses;
    };
    let Some(player) = world.get_resource::<PlayerEntityType>() else {
        return Filter::Refuses;
    };
    let wear = world
        .get::<Equippable>(query.stack)
        .filter(|equippable| player.admitted_by(equippable.allowed_entities.as_ref()))
        .and_then(|equippable| worn_slot(equippable.slot));
    rule.filter.place(Offer { wear })
}
