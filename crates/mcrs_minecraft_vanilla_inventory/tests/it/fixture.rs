use std::num::NonZeroU8;
use std::sync::LazyLock;

use bevy_app::App;
use bevy_app::Last;
use bevy_app::Update;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::Mut;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::dimension_keys::THE_NETHER;
use mcrs_minecraft_engine::ability::PlayerFlying;
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedRegistry;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::item::ItemStackValue;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_vanilla_inventory::InventoryPersistencePlugin;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

pub const STONE: Kind = Kind(1);
pub const DIRT: Kind = Kind(2);

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
    let mut app = gameplay_app();
    app.add_plugins(InventoryPersistencePlugin);
    app
}

pub fn gameplay_app() -> App {
    let mut app = App::new();
    app.insert_resource(CurrentRegistries::new(test_registries().clone()))
        .insert_resource(*TABLE)
        .add_plugins((ItemPlugin::default(), VanillaInventoryPlugin));
    app
}

pub fn saved(name: &str, count: i32) -> ItemStackValue<Named> {
    let mut saved = NbtCompound::new();
    saved.put("id", NbtTag::String(format!("minecraft:{name}")));
    saved.put("count", NbtTag::Int(count));
    mcrs_minecraft_nbt::from_tag(NbtTag::Compound(saved)).expect("a valid saved stack")
}

pub fn restore(world: &mut World, snapshot: &PersistedSnapshot) -> Entity {
    let player = world.spawn(Player).id();
    apply(world, player, snapshot);
    player
}

pub fn apply(world: &mut World, player: Entity, snapshot: &PersistedSnapshot) {
    world
        .resource_scope(|world, registry: Mut<PersistedRegistry>| {
            registry.apply(world, player, snapshot)
        })
        .expect("the snapshot applies");
}

pub fn capture(world: &World, player: Entity, cause: CaptureCause) -> PersistedSnapshot {
    world
        .resource::<PersistedRegistry>()
        .capture(world, player, cause)
        .expect("the player captures")
}

pub fn depart(world: &mut World, player: Entity, cause: CaptureCause) {
    let session = PlayerSession(1);
    world.init_resource::<DimPlayerIndex>();
    world.entity_mut(player).insert((
        Owner(session),
        EntityUuid(Uuid::nil()),
        Transform::IDENTITY,
        PlayerGameMode::default(),
        PlayerFlying(false),
        PlayerOpLevel(0),
        TeleportState::default(),
    ));
    match cause {
        CaptureCause::Leave => {
            world.write_message(InboundPlayerDespawn { session });
            world.run_schedule(Update);
        }
        CaptureCause::DimensionChange => {
            world.write_message(ChangeDimensionRequest {
                player,
                to: ResourceKeyBuf::from(THE_NETHER),
                transform: Transform::IDENTITY,
            });
            world.run_schedule(Last);
        }
        _ => {}
    }
}

pub fn capture_file(world: &World, player: Entity, cause: CaptureCause) -> PersistedSnapshot {
    world
        .resource::<PersistedRegistry>()
        .capture_file(world, player, cause)
        .expect("the player captures")
}

pub fn holders(world: &World, player: Entity) -> PlayerHolders {
    *world
        .get::<PlayerHolders>(player)
        .expect("a restored player has holders")
}

pub fn put(world: &mut World, holder: Entity, index: u16, kind: Kind, count: u8) {
    let count = Count::new(count).expect("a count above 0");
    world
        .commands()
        .spawn_stack(ItemStack::new(kind, count))
        .put_at(holder, index);
    world.flush();
}

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

pub fn patched_stone() -> ItemStack {
    let enchantment = EnchantmentLevel {
        enchantment: EnchantmentId(0),
        level: NonZeroU8::new(2).expect("a level above 0"),
    };
    ItemStack::new(STONE, Count::new(3).expect("a count above 0"))
        .set(CustomName(Text::text("Rock")))
        .set(Enchantments::new([enchantment]).expect("one enchantment"))
}

pub fn slot_stack(world: &mut World, holder: Entity, index: usize) -> Option<ItemStack> {
    let slots = world.get::<Slots>(holder).expect("a holder has slots");
    let stack = slots.iter().nth(index).flatten()?;
    world
        .run_system_once(move |mut stacks: Stacks| stacks.to_value(stack))
        .expect("the stack reads")
}

pub fn dropped(world: &mut World) -> Vec<(Kind, u8, i16)> {
    let mut dropped: Vec<(Kind, u8, i16)> = world
        .query_filtered::<(&Kind, &Count, &ItemEntity), With<Dropped>>()
        .iter(world)
        .map(|(kind, count, item)| (*kind, count.get(), item.pickup_delay))
        .collect();
    dropped.sort_unstable_by_key(|(kind, count, _)| (*kind, *count));
    dropped
}
