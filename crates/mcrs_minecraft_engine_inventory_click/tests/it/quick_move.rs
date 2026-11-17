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
use mcrs_minecraft_inventory_menu::MenuEntry;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Equippable;
use mcrs_minecraft_item_stack::EquippableSlot;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;

const BLOCK: Kind = Kind(0);
const HELMET: Kind = Kind(1);
const SHIELD: Kind = Kind(2);

fn moved_to(kind: Kind, from: u16, occupied: &[u16]) -> Vec<u16> {
    let mut app = App::new();
    let table = ItemTable::leak([
        vec![Builtin::MaxStackSize(MaxStackSize(64))],
        vec![
            Builtin::MaxStackSize(MaxStackSize(1)),
            Builtin::Equippable(Equippable::new(EquippableSlot::Head)),
        ],
        vec![
            Builtin::MaxStackSize(MaxStackSize(1)),
            Builtin::Equippable(Equippable::new(EquippableSlot::Offhand)),
        ],
    ]);
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
    let inventory_index = |entry: u16| MenuLayout::INVENTORY.entry(entry).unwrap().index;
    let mut commands = world.commands();
    commands
        .spawn_stack(ItemStack::new(kind, Count::new(1).unwrap()))
        .put_at(viewer, inventory_index(from));
    for &entry in occupied {
        commands
            .spawn_stack(ItemStack::new(BLOCK, Count::new(1).unwrap()))
            .put_at(viewer, inventory_index(entry));
    }
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
    let click = Click {
        kind: ClickKind::QuickMove,
        button: 0,
        clicked: Clicked::Entry(from),
    };
    let request = world
        .run_system_once(move |mut clicks: MenuClicks| clicks.plan(viewer, click, clicker))
        .unwrap()
        .unwrap();
    let entry_of = |slot: SlotRef| {
        assert_eq!(slot.holder, viewer, "a quick move stays in the inventory");
        let entry = MenuEntry {
            part: 0,
            index: slot.index,
        };
        MenuLayout::INVENTORY.index_of(entry).unwrap()
    };
    request
        .ops()
        .filter_map(|op| match op {
            Op::Split { to, .. } | Op::Merge { to, .. } => Some(entry_of(to)),
            Op::Swap { left, right } => Some(entry_of(left).max(entry_of(right))),
            Op::Set { .. } | Op::Drop { .. } => None,
        })
        .collect()
}

#[test]
fn inv_mnu_004_a_worn_stack_takes_the_route_of_its_equippable_slot() {
    let cases: [(&str, Kind, u16, &[u16], &[u16]); 5] = [
        ("helmet to an empty head entry", HELMET, 9, &[], &[5]),
        ("helmet from the hotbar", HELMET, 36, &[], &[5]),
        ("helmet with the head entry full", HELMET, 9, &[5], &[36]),
        ("offhand stack to the off hand", SHIELD, 9, &[], &[45]),
        ("block without equippable", BLOCK, 9, &[], &[36]),
    ];
    for (name, kind, from, occupied, expected) in cases {
        assert_eq!(moved_to(kind, from, occupied), expected, "{name}");
    }
}
