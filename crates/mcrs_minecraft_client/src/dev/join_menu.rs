use bevy::prelude::*;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuilders;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerLoaded;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

#[derive(Resource, Clone, Copy)]
struct JoinMenu {
    kind: MenuType,
    title_key: &'static str,
    open: fn(&mut App),
}

#[derive(Component)]
struct JoinMenuOpened;

struct Sample {
    slot: u16,
    item: &'static str,
    count: u8,
}

const JOIN_MENUS: [JoinMenu; 8] = [
    JoinMenu {
        kind: MenuType::Generic9x1,
        title_key: "container.chest",
        open: open_on_join::<0>,
    },
    JoinMenu {
        kind: MenuType::Generic9x2,
        title_key: "container.chest",
        open: open_on_join::<1>,
    },
    JoinMenu {
        kind: MenuType::Generic9x3,
        title_key: "container.chest",
        open: open_on_join::<2>,
    },
    JoinMenu {
        kind: MenuType::Generic9x4,
        title_key: "container.chest",
        open: open_on_join::<3>,
    },
    JoinMenu {
        kind: MenuType::Generic9x5,
        title_key: "container.chest",
        open: open_on_join::<4>,
    },
    JoinMenu {
        kind: MenuType::Generic9x6,
        title_key: "container.chestDouble",
        open: open_on_join::<5>,
    },
    JoinMenu {
        kind: MenuType::Generic3x3,
        title_key: "container.dispenser",
        open: open_on_join::<6>,
    },
    JoinMenu {
        kind: MenuType::Hopper,
        title_key: "container.hopper",
        open: open_on_join::<7>,
    },
];

const CONTAINER_STACKS: &[Sample] = &[
    Sample {
        slot: 0,
        item: "minecraft:stone",
        count: 64,
    },
    Sample {
        slot: 1,
        item: "minecraft:dirt",
        count: 10,
    },
    Sample {
        slot: 2,
        item: "minecraft:diamond_sword",
        count: 1,
    },
    Sample {
        slot: 4,
        item: "minecraft:oak_planks",
        count: 16,
    },
    Sample {
        slot: 8,
        item: "minecraft:cobblestone",
        count: 32,
    },
    Sample {
        slot: 13,
        item: "minecraft:torch",
        count: 5,
    },
    Sample {
        slot: 26,
        item: "minecraft:apple",
        count: 3,
    },
    Sample {
        slot: 53,
        item: "minecraft:gold_ingot",
        count: 7,
    },
];

#[must_use]
pub fn join_menu_server(kind: MenuType) -> Option<fn(&mut App)> {
    JOIN_MENUS
        .iter()
        .find(|menu| menu.kind == kind)
        .map(|menu| menu.open)
}

fn open_on_join<const INDEX: usize>(server: &mut App) {
    let Some(menu) = JOIN_MENUS.get(INDEX) else {
        return;
    };
    server.insert_resource(*menu);
    server
        .world_mut()
        .get_resource_or_init::<DimensionBuilders>()
        .add(add_join_menu);
}

fn add_join_menu(host: &World, _: &DimensionBuild<'_>, dimension: &mut SubApp) {
    if let Some(menu) = host.get_resource::<JoinMenu>() {
        dimension
            .insert_resource(*menu)
            .add_observer(open_join_menu);
    }
}

fn open_join_menu(
    loaded: On<Received<ServerboundPlayerLoaded>>,
    menu: Res<JoinMenu>,
    items: Res<ItemTable>,
    opened: Query<(), With<JoinMenuOpened>>,
    mut commands: Commands,
) {
    let player = loaded.entity;
    if opened.contains(player) {
        return;
    }
    let Some(layout) = MenuLayout::of(menu.kind) else {
        return;
    };
    let size = layout.part_len(MenuLayout::GENERIC_CONTAINER);
    let container = commands.spawn(Slots::new(usize::from(size))).id();
    let Some(parts) = MenuParts::new([container, player]) else {
        commands.entity(container).despawn();
        return;
    };
    for sample in CONTAINER_STACKS {
        if sample.slot < size {
            put_sample(&mut commands, &items, sample, container);
        }
    }
    commands
        .entity(player)
        .insert(JoinMenuOpened)
        .open_titled_menu(
            layout,
            parts,
            MenuButtons::default(),
            MenuTitle(Text::translate(menu.title_key, Vec::new())),
        );
}

fn put_sample(commands: &mut Commands, items: &ItemTable, sample: &Sample, holder: Entity) {
    let Ok(location) = ResourceLocation::read(sample.item) else {
        return;
    };
    let Some(id) = items.id(NameTable::Item, location) else {
        return;
    };
    let Some(count) = Count::new(sample.count) else {
        return;
    };
    commands
        .spawn_stack(ItemStack::new(Kind(id), count))
        .put_at(holder, sample.slot);
}
