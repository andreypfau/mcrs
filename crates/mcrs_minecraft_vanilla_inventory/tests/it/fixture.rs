use std::sync::LazyLock;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::world::Mut;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedRegistry;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol_game::item::ItemStackValue;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_vanilla_inventory::InventoryPersistencePlugin;
use mcrs_minecraft_vanilla_inventory::PlayerHolders;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

pub const STONE: Kind = Kind(1);
pub const DIRT: Kind = Kind(2);

/// The items `stone` and `dirt` after `air`, each with the maximum stack size 64.
static TABLE: LazyLock<ItemTable> = LazyLock::new(|| {
    let files = ["air", "stone", "dirt"].iter().zip(0..).map(|(name, id)| {
        format!(
            r#"{{"minecraft:item":{{"description":{{"identifier":"minecraft:{name}","protocol_id":{id}}},"components":{{"minecraft:max_stack_size":64}}}}}}"#
        )
    });
    let files: Vec<String> = files.collect();
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

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        ItemPlugin::new(*TABLE),
        VanillaInventoryPlugin,
        InventoryPersistencePlugin,
    ));
    app
}

/// The value of the player file for `count` items of `minecraft:<name>`.
pub fn saved(name: &str, count: i32) -> ItemStackValue<Named> {
    let mut saved = NbtCompound::new();
    saved.put("id", NbtTag::String(format!("minecraft:{name}")));
    saved.put("count", NbtTag::Int(count));
    mcrs_minecraft_nbt::from_tag(NbtTag::Compound(saved)).expect("a valid saved stack")
}

/// Spawns a player and applies `snapshot` to it, as the engine does at a join or an arrival.
pub fn restore(world: &mut World, snapshot: &PersistedSnapshot) -> Entity {
    let player = world.spawn(Player).id();
    world
        .resource_scope(|world, registry: Mut<PersistedRegistry>| {
            registry.apply(world, player, snapshot)
        })
        .expect("the snapshot applies");
    player
}

/// What a handover of `player` carries to another world.
pub fn capture(world: &mut World, player: Entity, cause: CaptureCause) -> PersistedSnapshot {
    world
        .resource_scope(|world, registry: Mut<PersistedRegistry>| {
            registry.capture(world, player, cause)
        })
        .expect("the player captures")
}

/// What the player file of `player` holds.
pub fn capture_file(world: &mut World, player: Entity, cause: CaptureCause) -> PersistedSnapshot {
    world
        .resource_scope(|world, registry: Mut<PersistedRegistry>| {
            registry.capture_file(world, player, cause)
        })
        .expect("the player captures")
}

pub fn holders(world: &World, player: Entity) -> PlayerHolders {
    *world
        .get::<PlayerHolders>(player)
        .expect("a restored player has holders")
}

/// Puts a new stack of `count` items of `kind` in the slot `index` of `holder`.
pub fn put(world: &mut World, holder: Entity, index: u16, kind: Kind, count: u8) {
    let count = Count::new(count).expect("a count above 0");
    world
        .commands()
        .spawn_stack(ItemStack::new(kind, count))
        .put_at(holder, index);
    world.flush();
}

/// The slot index, the item and the count of each stack of `holder`.
pub fn stacks(world: &World, holder: Entity) -> Vec<(usize, Kind, u8)> {
    let slots = world.get::<Slots>(holder).expect("a holder has slots");
    slots
        .iter()
        .enumerate()
        .filter_map(|(index, stack)| {
            let stack = world.entity(stack?);
            let kind = *stack.get::<Kind>().expect("a stack has a kind");
            let count = stack.get::<Count>().expect("a stack has a count").get();
            Some((index, kind, count))
        })
        .collect()
}

/// The item, the count and the pickup delay of each stack that lies in the world.
pub fn dropped(world: &mut World) -> Vec<(Kind, u8, i16)> {
    let mut dropped: Vec<(Kind, u8, i16)> = world
        .query_filtered::<(&Kind, &Count, &ItemEntity), With<Dropped>>()
        .iter(world)
        .map(|(kind, count, item)| (*kind, count.get(), item.pickup_delay))
        .collect();
    dropped.sort_unstable_by_key(|(kind, count, _)| (*kind, *count));
    dropped
}
