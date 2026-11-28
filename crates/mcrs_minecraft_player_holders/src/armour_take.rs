use bevy_ecs::world::World;
use mcrs_minecraft_enchantment::enchantment_effect_component_type_keys::EnchantmentEffectComponentType;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_inventory_player::ArmourLock;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_item_stack::Enchantments;

use crate::enchantment_effects::EnchantmentEffects;

pub(crate) fn armour_take(world: &World, query: &GateQuery) -> Filter {
    let Some(rule) = Holder::Inventory.slot(query.slot.index) else {
        return Filter::Refuses;
    };
    let effects = world.get_resource::<EnchantmentEffects>();
    let lock = match (effects, world.get::<Enchantments>(query.stack)) {
        (Some(effects), Some(enchantments))
            if effects.any_has(
                enchantments,
                EnchantmentEffectComponentType::PreventArmorChange,
            ) =>
        {
            ArmourLock::Locked
        }
        _ => ArmourLock::Free,
    };
    let mode = match world.get::<PlayerGameMode>(query.actor) {
        Some(PlayerGameMode(GameMode::Creative)) => PlayerMode::Creative,
        _ => PlayerMode::NotCreative,
    };
    rule.filter.take(lock, mode)
}
