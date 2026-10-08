use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_input::ButtonInput;
use bevy_input::keyboard::KeyCode;
use bytes::Bytes;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_client::columns::{BlockSource, ColumnStore};
use mcrs_minecraft_client::inventory::{ContainerSeqno, InventoryPlugin, inventory_index_to_cell};
use mcrs_minecraft_client::player::Player;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_item::{ItemStack, SelectedHotbarSlot, SlotTable, slots};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::Instant as PacketInstant;
use mcrs_minecraft_network::client::{
    ChunkCacheCenter, ChunkCacheRadius, CurrentDimension, JoinedGame, PendingTeleports,
    SessionRegistryInputs,
};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::item::{ComponentPatch, RawStack};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSetPlayerInventory;
use mcrs_minecraft_protocol::{Encode, Packet, ProtoStack, VarInt};
use mcrs_minecraft_registry::{Id, RegistrySet};
use std::time::{Duration, Instant};

pub const JOIN_TIMEOUT: Duration = Duration::from_secs(120);

/// A column cannot be decoded without the number of block states: it fixes the
/// width a section's states are packed at once the section drops its palette.
pub fn insert_block_catalog(client: &mut App) {
    client.insert_resource(mcrs_minecraft_worldgen_generator::tests::blocks().clone());
}

/// What the client reads the server's registries with once configuration ends, the vanilla
/// pack's entries included, as the client's registry plugin holds them.
pub fn insert_session_inputs(client: &mut App) {
    client.insert_resource(mcrs_minecraft_client::registries::session_inputs(Some(
        &mcrs_minecraft_client::asset_corpus(),
    )));
}

pub fn insert_inventory(client: &mut App) {
    client
        .insert_resource(mcrs_minecraft_world::item::test_corpus().1.clone())
        .init_resource::<ButtonInput<KeyCode>>()
        .add_plugins(InventoryPlugin);
    client.world_mut().spawn((
        Player,
        SlotTable::fixed(slots::COUNT),
        SelectedHotbarSlot::default(),
        ContainerSeqno::default(),
    ));
}

/// Returns the connection entity once every play-state packet the flow promises
/// has arrived, or `None` if the deadline passes first.
pub fn drive_client_until_joined(client: &mut App) -> Option<bevy_ecs::entity::Entity> {
    let deadline = Instant::now() + JOIN_TIMEOUT;
    let mut seen_configuration = false;

    while Instant::now() < deadline {
        client.update();

        let world = client.world_mut();
        let mut connections = world.query::<(bevy_ecs::entity::Entity, &ConnectionState)>();
        let Some((entity, state)) = connections.iter(world).next() else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        seen_configuration |= *state == ConnectionState::Configuration;
        if *state != ConnectionState::Game {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }

        assert!(
            seen_configuration,
            "the connection reached play without passing through configuration"
        );
        let joined = world.get::<JoinedGame>(entity).is_some();
        let positioned = world
            .get::<PendingTeleports>(entity)
            .is_some_and(|teleports| !teleports.0.is_empty());
        let centred = world.get::<ChunkCacheCenter>(entity).is_some();
        let radius = world.get::<ChunkCacheRadius>(entity).is_some();
        let chunks = !world.resource::<ColumnStore>().is_empty();
        if joined && positioned && centred && radius && chunks {
            return Some(entity);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

pub fn assert_session_registries(client: &App, connection: Entity) {
    let world = client.world();
    let session = world.resource::<RegistrySet>();
    let inputs = world.resource::<SessionRegistryInputs>();

    let declared: Vec<&str> = inputs
        .declarations
        .synced()
        .map(|registry| registry.as_str())
        .collect();
    assert!(
        !declared.is_empty(),
        "the client declares no synced registry"
    );
    for registry in declared {
        assert!(
            session.table(registry).is_some(),
            "the session holds no {registry}"
        );
    }

    let biomes = session
        .registry::<Biome>()
        .expect("the session holds the biome registry");
    let biome_values = session
        .entries::<Biome, Biome>()
        .expect("the session holds the biome values");
    assert!(!biomes.is_empty(), "the biome registry arrived empty");
    assert_eq!(biome_values.as_slice().len(), biomes.len());
    assert!(
        biome_values
            .as_slice()
            .iter()
            .any(|biome| biome.effects.water_color.is_some()),
        "no biome carries a water colour"
    );

    for registry in ["minecraft:block", "minecraft:worldgen/biome"] {
        let tags = session
            .tag_table(registry)
            .unwrap_or_else(|| panic!("the session holds no tags of {registry}"));
        assert!(!tags.is_empty(), "the tags of {registry} arrived empty");
    }

    let current = world
        .get::<CurrentDimension>(connection)
        .expect("the login named the dimension the player is in");
    let dimension_types = session
        .entries::<DimensionType, DimensionType>()
        .expect("the session holds the dimension type values");
    let dimension_type = dimension_types
        .get(current.dimension_type)
        .expect("the login names a dimension type of the session");
    let extent = world
        .resource::<ColumnStore>()
        .extent()
        .expect("the store holds the extent of the dimension");
    assert_eq!(
        u32::try_from(extent.sections).unwrap() * 16,
        dimension_type.height,
        "the column height is the dimension type's"
    );
    assert_eq!(
        extent.min_section_y * 16,
        dimension_type.min_y,
        "the column floor is the dimension type's"
    );
}

pub fn assert_inventory_resolves_through_session(client: &mut App, connection: Entity) {
    let session = client.world().resource::<RegistrySet>().clone();
    let items = session
        .registry::<Item>()
        .expect("the session holds the item registry");
    let ids: Vec<Id<Item>> = items.ids().collect();
    let sent: Vec<(i32, Id<Item>, i32)> = [1, ids.len() / 2, ids.len() - 1]
        .into_iter()
        .enumerate()
        .map(|(index, position)| (index as i32, ids[position], 2 + index as i32))
        .collect();

    let world = client.world_mut();
    for &(index, item, count) in &sent {
        let stack = ProtoStack::new(item, count, ComponentPatch::EMPTY);
        let packet = ClientboundSetPlayerInventory {
            slot: VarInt(index),
            contents: RawStack::from_stack(&stack, &session).expect("the item is in the session"),
        };
        let mut data = Vec::new();
        packet.encode(&mut data).expect("the packet encodes");
        world.trigger(ReceivedPacketEvent {
            entity: connection,
            id: ClientboundSetPlayerInventory::ID,
            data: Bytes::from(data),
            timestamp: PacketInstant::now(),
        });
    }
    world.flush();

    let player = world
        .query_filtered::<Entity, With<Player>>()
        .single(world)
        .expect("the client has one player");
    for (index, item, count) in sent {
        let cell = inventory_index_to_cell(index).expect("a cell of the player's inventory");
        let entity = world
            .get::<SlotTable>(player)
            .unwrap()
            .get(cell)
            .unwrap_or_else(|| panic!("inventory slot {index} holds no stack"));
        let stack = *world.get::<ItemStack>(entity).unwrap();
        assert_eq!(items.name(stack.item), items.name(item), "slot {index}");
        assert_eq!(i32::from(stack.count), count, "slot {index}");
    }
    for stack in world.query::<&ItemStack>().iter(world) {
        assert!(
            items.name(stack.item).is_some(),
            "a stack names an item the session does not hold"
        );
    }
}
