use bevy_app::App;
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_engine_inventory_menu::EntryPressed;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_inventory_minigame::OpenShop;
use mcrs_minecraft_inventory_minigame::ShopPlugin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_vanilla_inventory::PlayerInventories;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

#[test]
fn pressing_the_offer_in_the_shop_gives_the_player_the_offered_stack() {
    let mut app = App::new();
    app.insert_resource(ItemTable::leak((0..5).map(|_| Vec::new())))
        .add_plugins((ItemPlugin::default(), VanillaInventoryPlugin, ShopPlugin));
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();

    world.trigger(OpenShop { entity: player });
    world.flush();
    let menus = world.get::<Menus>(player).unwrap();
    assert_eq!(
        menus.iter().count(),
        2,
        "the shop opens above the first menu"
    );
    let menu = menus.open().unwrap();
    let holder = world.get::<MenuParts>(menu).unwrap().holder(0).unwrap();
    assert!(world.get::<MenuButtons>(menu).unwrap().contains(13));

    world.write_message(EntryPressed {
        viewer: player,
        menu,
        entry: 13,
        button: 0,
    });
    app.update();

    let world = app.world_mut();
    let items: PlayerItems = world
        .run_system_once(move |mut inventories: PlayerInventories| inventories.capture(player))
        .unwrap()
        .unwrap();
    let offered = ItemStack::new(Kind(3), Count::new(1).unwrap());
    assert!(
        items.slots.contains(&Some(offered)),
        "the player holds the offered stack"
    );

    let world = app.world_mut();
    world.commands().entity(player).close_menu();
    world.flush();
    assert!(
        world.get_entity(holder).is_err(),
        "closing the shop despawns its holder"
    );
}

#[test]
fn the_holder_of_the_shop_goes_with_the_menu_when_the_player_leaves() {
    let mut app = App::new();
    app.insert_resource(ItemTable::leak((0..5).map(|_| Vec::new())))
        .add_plugins((ItemPlugin::default(), VanillaInventoryPlugin, ShopPlugin));
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();
    world.trigger(OpenShop { entity: player });
    world.flush();
    let menu = world.get::<Menus>(player).unwrap().open().unwrap();
    let holder = world.get::<MenuParts>(menu).unwrap().holder(0).unwrap();

    world.despawn(player);
    world.flush();

    assert!(world.get_entity(holder).is_err());
}
