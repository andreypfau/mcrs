use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_inventory_minigame::EnterGame;
use mcrs_minecraft_inventory_minigame::KitPlugin;
use mcrs_minecraft_inventory_minigame::KitSnapshot;
use mcrs_minecraft_inventory_minigame::LeaveGame;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_vanilla_inventory::PlayerInventories;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

#[test]
fn a_player_holds_the_kit_in_the_game_and_its_own_items_after_it() {
    let stack = |kind: u16, count: u8| ItemStack::new(Kind(kind), Count::new(count).unwrap());
    let capture = |world: &mut World, player: Entity| -> PlayerItems {
        world
            .run_system_once(move |mut inventories: PlayerInventories| inventories.capture(player))
            .unwrap()
            .unwrap()
    };
    let mut kit = PlayerItems::default();
    kit.slots[0] = Some(stack(3, 1));
    kit.slots[39] = Some(stack(4, 1));
    let mut own = PlayerItems {
        selected: 6,
        ..PlayerItems::default()
    };
    own.slots[2] = Some(stack(1, 12));
    own.slots[40] = Some(stack(2, 64));
    let mut app = App::new();
    app.insert_resource(ItemTable::leak((0..5).map(|_| Vec::new())))
        .add_plugins((
            ItemPlugin::default(),
            VanillaInventoryPlugin,
            KitPlugin::new(kit.clone()),
        ));
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.commands().entity(player).restore_items(own.clone());
    world.flush();

    world.trigger(EnterGame { entity: player });
    world.flush();
    assert_eq!(
        capture(world, player),
        kit,
        "the player holds the kit alone"
    );

    world.trigger(LeaveGame { entity: player });
    world.flush();
    assert_eq!(
        capture(world, player),
        own,
        "the player holds its own stacks and selected slot again"
    );
    assert!(
        world.get::<KitSnapshot>(player).is_none(),
        "a player out of the game keeps no snapshot"
    );
}
