use bevy_app::App;
use bevy_app::Plugin;
use mcrs_minecraft_engine_inventory_menu::MenuPlugin;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::ItemAppExt;

use crate::armour_gates::ArmourGates;
use crate::armour_place::armour_place;
use crate::armour_take::armour_take;
use crate::enchantment_effects_plugin::EnchantmentEffectsPlugin;
use crate::player_entity_type::PlayerEntityType;
use crate::player_entity_type::derive_player_entity_type;
use crate::player_entity_type::forget_player_entity_type;
use crate::restore_holders::hold_new_player;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerHoldersPlugin;

impl Plugin for PlayerHoldersPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MenuPlugin>() {
            app.add_plugins(MenuPlugin);
        }
        if !app.is_plugin_added::<EnchantmentEffectsPlugin>() {
            app.add_plugins(EnchantmentEffectsPlugin);
        }
        let place = match app.register_slot_gate(armour_place) {
            Ok(id) => Gate::Ask(id),
            Err(error) => {
                tracing::error!("an armour slot refuses each stack: {error}");
                Gate::Refuses
            }
        };
        let take = match app.register_slot_gate(armour_take) {
            Ok(id) => Gate::Ask(id),
            Err(error) => {
                tracing::error!("an armour slot refuses each take: {error}");
                Gate::Refuses
            }
        };
        app.insert_resource(ArmourGates { place, take })
            .init_resource::<PlayerEntityType>()
            .add_observer(derive_player_entity_type)
            .add_observer(forget_player_entity_type)
            .add_observer(hold_new_player);
    }
}
