use bevy_app::App;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_inventory_minigame::ArenaRulesPlugin;
use mcrs_minecraft_inventory_minigame::KitPlugin;
use mcrs_minecraft_inventory_minigame::ShopPlugin;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

fn main() {
    App::new()
        .insert_resource(ItemTable::leak([]))
        .add_plugins((
            ItemPlugin::default(),
            VanillaInventoryPlugin,
            KitPlugin::new(PlayerItems::default()),
            ArenaRulesPlugin,
            ShopPlugin,
        ))
        .run();
}
