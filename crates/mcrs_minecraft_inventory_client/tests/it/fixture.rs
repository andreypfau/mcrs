use std::sync::LazyLock;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_inventory_client::InventoryClientPlugin;
use mcrs_minecraft_inventory_client::RemoteMenu;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_player_holders::PlayerHoldersPlugin;
use mcrs_minecraft_player_holders::restore_holders;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;

pub(crate) const STONE: Kind = Kind(1);
pub(crate) const DIRT: Kind = Kind(2);

static TABLE: LazyLock<ItemTable> = LazyLock::new(|| {
    let files: Vec<String> = ["air", "stone", "dirt"]
        .iter()
        .zip(0..)
        .map(|(name, id)| {
            format!(
                r#"{{"minecraft:item":{{"description":{{"identifier":"minecraft:{name}","protocol_id":{id}}},"components":{{"minecraft:max_stack_size":64}}}}}}"#
            )
        })
        .collect();
    bake(
        files.iter().map(|bytes| DefinitionFile {
            name: "test",
            bytes: bytes.as_bytes(),
        }),
        &ItemTable::leak([]),
    )
    .expect("the test items bake")
    .table
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Player,
    CraftGrid,
    CraftResult,
}

pub(crate) fn holder_slot_of_entry(entry: u16) -> (Part, u16) {
    match entry {
        0 => (Part::CraftResult, 0),
        1..=4 => (Part::CraftGrid, entry - 1),
        5..=8 => (Part::Player, 44 - entry),
        9..=35 => (Part::Player, entry),
        36..=44 => (Part::Player, entry - 36),
        _ => (Part::Player, 40),
    }
}

pub(crate) fn client_with_player() -> (App, Entity) {
    let mut app = App::new();
    app.insert_resource(CurrentRegistries::new(test_registries().clone()))
        .insert_resource(*TABLE)
        .add_plugins((
            ItemPlugin::default(),
            PlayerHoldersPlugin,
            InventoryClientPlugin,
        ));
    let world = app.world_mut();
    let player = world.spawn_empty().id();
    world
        .run_system_cached_with(restore_holders, (player, PlayerItems::default()))
        .expect("the holders restore");
    world.flush();
    (app, player)
}

pub(crate) fn stack(kind: Kind, count: u8) -> ItemStack {
    ItemStack::new(kind, Count::new(count).expect("a count above 0"))
}

pub(crate) fn wire(world: &World, stack: Option<&ItemStack>) -> ProtoStack<Wire> {
    let names = RegistryNames::new(
        world.resource::<CurrentRegistries>(),
        world.resource::<ItemTable>(),
    );
    let mut bytes = Vec::new();
    write_wire(stack, &names, &mut bytes).expect("the stack has a wire form");
    ProtoStack::decode(&mut bytes.as_slice()).expect("the wire form decodes")
}

fn stack_in(world: &World, holder: Entity, index: u16) -> Option<(Kind, u8)> {
    let slots = world.get::<Slots>(holder)?;
    let stack = world.entity(slots.get(usize::from(index))?);
    Some((*stack.get::<Kind>()?, stack.get::<Count>()?.get()))
}

pub(crate) fn content(world: &World, entry: u16) -> Option<(Kind, u8)> {
    let (part, index) = holder_slot_of_entry(entry);
    stack_in(world, holder(world, part), index)
}

pub(crate) fn cursor(world: &World) -> Option<(Kind, u8)> {
    stack_in(world, holders(world).1.cursor, 0)
}

pub(crate) fn stacks(world: &World) -> usize {
    let (player, holders) = holders(world);
    [
        player,
        holders.cursor,
        holders.craft_grid,
        holders.craft_result,
    ]
    .iter()
    .filter_map(|holder| world.get::<Slots>(*holder))
    .map(|slots| slots.occupied().count())
    .sum()
}

pub(crate) fn state_id(world: &World) -> Option<i32> {
    let (player, _) = holders(world);
    let menu = world
        .get::<Menus>(player)
        .and_then(|menus| menus.iter().next())
        .expect("the inventory menu is open");
    world.get::<RemoteMenu>(menu).map(|remote| {
        assert_eq!(
            remote.container_id(),
            0,
            "only the inventory menu is remote"
        );
        remote.state_id()
    })
}

fn holders(world: &World) -> (Entity, PlayerHolders) {
    let mut players = world
        .try_query::<(Entity, &PlayerHolders)>()
        .expect("the holders are registered");
    let (player, holders) = players.single(world).expect("one local player");
    (player, *holders)
}

fn holder(world: &World, part: Part) -> Entity {
    let (player, holders) = holders(world);
    match part {
        Part::Player => player,
        Part::CraftGrid => holders.craft_grid,
        Part::CraftResult => holders.craft_result,
    }
}

pub(crate) fn body<P: Encode>(packet: &P) -> Vec<u8> {
    let mut body = Vec::new();
    packet.encode(&mut body).expect("the packet encodes");
    body
}
