use bevy_app::App;
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_inventory_click::MenuClickPlugin;
use mcrs_minecraft_engine_inventory_click::MenuClicker;
use mcrs_minecraft_engine_inventory_click::MenuClicks;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuPlugin;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;

fn drag(button: u8, clicked: Clicked) -> Click {
    Click {
        kind: ClickKind::Drag,
        button,
        clicked,
    }
}

#[test]
fn two_drag_clicks_in_one_system_see_each_other() {
    let mut app = App::new();
    let table = ItemTable::leak([vec![Builtin::MaxStackSize(MaxStackSize(64))]]);
    app.insert_resource(table)
        .add_plugins((ItemPlugin::default(), MenuPlugin, MenuClickPlugin));
    let source = app.allocate_request_source().unwrap();
    let world = app.world_mut();
    let viewer = world
        .spawn(Slots::new(usize::from(Holder::Inventory.slot_count())))
        .id();
    let craft_grid = world.spawn(Slots::new(4)).id();
    let craft_result = world.spawn(Slots::new(1)).id();
    let cursor = world.spawn(Slots::new(1)).id();
    let parts = MenuParts::new([viewer, craft_grid, craft_result]).unwrap();
    let mut commands = world.commands();
    commands
        .spawn_stack(ItemStack::new(Kind(0), Count::new(8).unwrap()))
        .put_at(cursor, 0);
    commands
        .entity(viewer)
        .open_menu(MenuLayout::INVENTORY, parts, MenuButtons::default());
    world.flush();
    let clicker = MenuClicker {
        player: ClickPlayer::default(),
        cursor: SlotRef {
            holder: cursor,
            index: 0,
        },
        request: RequestId { source, serial: 1 },
    };

    let ends = world
        .run_system_once(move |mut clicks: MenuClicks| {
            let clicks = [
                drag(0, Clicked::Outside),
                drag(1, Clicked::Entry(36)),
                drag(1, Clicked::Entry(37)),
                drag(2, Clicked::Outside),
            ]
            .map(|click| clicks.plan(viewer, click, clicker).unwrap());
            clicks.map(|request| request.ops().count())
        })
        .unwrap();

    assert_eq!(ends, [0, 0, 0, 2]);
}
