use std::collections::BTreeMap;

use bevy::input::ButtonInput;
use bevy::prelude::*;
use bytes::Bytes;
use mcrs_minecraft_client::blocks::BiomeTints;
use mcrs_minecraft_client::columns::{
    BlockSource, Column, ColumnCachePlugin, ColumnStore, Dimension, Extent,
};
use mcrs_minecraft_client::inventory::{InventoryPlugin, OpenMenu, Screen};
use mcrs_minecraft_client::player::{Player, PlayerCamera};
use mcrs_minecraft_client::sky::{SkyEnvironment, SkyPlugin};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::environment::Weather;
use mcrs_minecraft_environment::world_clock::{ClockTimeMarkers, WorldClock, WorldClocks};
use mcrs_minecraft_item::{Held, ItemStack, Items, JukeboxPlayable, JukeboxSong, SlotTable, slots};
use mcrs_minecraft_network::client::CurrentDimension;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, Instant};
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundRespawn;
use mcrs_minecraft_protocol::{Encode, Packet, RegistryId};
use mcrs_minecraft_registry::{Holder, Id, NetworkRegistry, RegistrySet};

use crate::boot::{boot, session_registries};

fn play_app() -> App {
    boot(|app| {
        app.add_plugins((ColumnCachePlugin, InventoryPlugin, SkyPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ClearColor>()
            .insert_resource(Weather::default());
    })
}

fn reversed(registry: &'static str) -> impl FnOnce(&mut Vec<NetworkRegistry>) {
    move |sent| {
        sent.iter_mut()
            .find(|sent| sent.registry.as_str() == registry)
            .unwrap_or_else(|| panic!("a server syncs {registry}"))
            .entries
            .reverse();
    }
}

fn disc_song(items: &Items) -> Id<JukeboxSong> {
    let disc = items
        .id_of("minecraft:music_disc_13")
        .expect("the item corpus holds the first music disc");
    let song = items
        .get(disc)
        .and_then(|entry| entry.prototype.get::<JukeboxPlayable>())
        .expect("a music disc names its song");
    let Holder::Reference(song) = &song.0.0 else {
        panic!("the corpus names the song by reference");
    };
    *song
}

fn dimension_type(registries: &RegistrySet, name: &str) -> Id<DimensionType> {
    registries
        .registry::<DimensionType>()
        .expect("the dimension types are received")
        .require_by_name(name)
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn current(name: &str, dimension_type: Id<DimensionType>) -> CurrentDimension {
    CurrentDimension {
        key: ResourceKey::from_location(ResourceLocation::read(name).unwrap()),
        dimension_type,
    }
}

fn sky_key(app: &App) -> Option<String> {
    let environment = app.world().get_resource::<SkyEnvironment>()?;
    Some(format!("{:?}", environment.key()))
}

fn markers_by_clock(app: &App) -> BTreeMap<String, usize> {
    let clocks = app
        .world()
        .resource::<RegistrySet>()
        .registry::<WorldClock>()
        .unwrap();
    let markers = app.world().resource::<ClockTimeMarkers>();
    clocks
        .ids()
        .map(|clock| {
            let name = clocks.name(clock).unwrap().to_string();
            (name, markers.of_clock(clock).count())
        })
        .collect()
}

fn set_connection_state(app: &mut App, connection: Entity, state: ConnectionState) {
    *app.world_mut()
        .get_mut::<ConnectionState>(connection)
        .unwrap() = state;
    app.update();
}

fn receive_respawn(app: &mut App, connection: Entity, dimension: &str, dimension_type: u16) {
    let packet = ClientboundRespawn {
        player_spawn_info: PlayerSpawnInfo {
            dimension_type_id: RegistryId(dimension_type),
            dimension: ResourceKey::from_location(ResourceLocation::read(dimension).unwrap()),
            ..Default::default()
        },
        data_to_keep: 0,
    };
    let mut bytes = Vec::new();
    packet.encode(&mut bytes).unwrap();
    app.world_mut().trigger(ReceivedPacketEvent {
        entity: connection,
        id: ClientboundRespawn::ID,
        data: Bytes::from(bytes),
        timestamp: Instant::now(),
    });
    app.world_mut().flush();
    app.update();
}

fn column(extent: Extent) -> Column {
    Column::unlit(extent.min_section_y, vec![None; extent.sections])
}

const OVERWORLD_EXTENT: Extent = Extent {
    min_section_y: -4,
    sections: 24,
};

fn overworld() -> Dimension {
    Dimension {
        name: "minecraft:overworld".to_owned(),
        extent: OVERWORLD_EXTENT,
    }
}

struct Play {
    app: App,
    connection: Entity,
    menu: Entity,
    stacks: [Entity; 2],
}

fn join(app: &mut App, connection: Entity) {
    let registries = session_registries(app, |_| {});
    let overworld_type = dimension_type(&registries, "minecraft:overworld");
    app.insert_resource(registries);
    app.world_mut()
        .entity_mut(connection)
        .insert(current("minecraft:overworld", overworld_type));
    app.update();
}

fn in_play() -> Play {
    let mut app = play_app();
    app.world_mut()
        .spawn((PlayerCamera, GlobalTransform::from_xyz(0.0, 80.0, 0.0)));
    let player = app
        .world_mut()
        .spawn((Player, SlotTable::fixed(slots::COUNT)))
        .id();
    let connection = app.world_mut().spawn(ConnectionState::Game).id();
    join(&mut app, connection);

    let menu = app
        .world_mut()
        .spawn((OpenMenu { container_id: 1 }, SlotTable::fixed(3)))
        .id();
    *app.world_mut().resource_mut::<Screen>() = Screen::Container(menu);
    let stack = |holder| {
        (
            ItemStack::new(Id::from_raw(0), 1),
            Held { holder, index: 0 },
        )
    };
    let stacks = [
        app.world_mut().spawn(stack(menu)).id(),
        app.world_mut().spawn(stack(player)).id(),
    ];

    let mut store = app.world_mut().resource_mut::<ColumnStore>();
    store.enter(overworld());
    store.insert(ColumnPos::new(0, 0), column(OVERWORLD_EXTENT));
    store.insert(ColumnPos::new(1, 0), column(OVERWORLD_EXTENT));
    app.world_mut().insert_resource(BiomeTints::default());
    app.update();
    Play {
        app,
        connection,
        menu,
        stacks,
    }
}

use mcrs_minecraft_protocol::ColumnPos;

#[test]
fn entering_configuration_clears_the_play_world() {
    let Play {
        mut app,
        connection,
        menu,
        stacks,
    } = in_play();
    let before = (
        app.world().resource::<Items>().len(),
        disc_song(app.world().resource::<Items>()),
        markers_by_clock(&app),
        app.world().resource::<WorldClocks>().len(),
        sky_key(&app).expect("the sky is built in play"),
    );
    assert!(before.2.values().any(|count| *count > 0) && before.3 > 0);
    assert_eq!(app.world().resource::<ColumnStore>().len(), 2);

    set_connection_state(&mut app, connection, ConnectionState::Configuration);

    let world = app.world();
    let store = world.resource::<ColumnStore>();
    assert!(store.is_empty(), "the stored columns are gone");
    assert!(store.extent().is_none(), "and the dimension they sat in");
    assert_eq!(*world.resource::<Screen>(), Screen::None);
    assert!(world.get_entity(menu).is_err(), "the open menu is closed");
    for stack in stacks {
        assert!(world.get_entity(stack).is_err(), "{stack:?} is despawned");
    }
    assert!(world.get_resource::<SkyEnvironment>().is_none());
    assert!(world.get_resource::<Items>().is_none());
    assert!(world.get_resource::<ClockTimeMarkers>().is_none());
    assert!(world.get_resource::<BiomeTints>().is_none());
    assert!(world.resource::<WorldClocks>().is_empty());
    assert!(world.get::<CurrentDimension>(connection).is_none());

    app.update();
    app.update();
    assert!(
        app.world().get_resource::<Items>().is_none(),
        "the previous set is still there, but nothing is built from it again"
    );
    assert!(app.world().get_resource::<ClockTimeMarkers>().is_none());
    assert!(app.world().resource::<WorldClocks>().is_empty());

    set_connection_state(&mut app, connection, ConnectionState::Game);
    join(&mut app, connection);
    let after = (
        app.world().resource::<Items>().len(),
        disc_song(app.world().resource::<Items>()),
        markers_by_clock(&app),
        app.world().resource::<WorldClocks>().len(),
        sky_key(&app).expect("the login names the dimension again"),
    );
    assert_eq!(
        after, before,
        "a set with the names of the one it replaced gives the tables of a fresh join"
    );
}

#[test]
fn a_respawn_into_another_dimension_replaces_the_client_world() {
    let Play {
        mut app,
        connection,
        ..
    } = in_play();
    let registries = app.world().resource::<RegistrySet>().clone();
    let nether = dimension_type(&registries, "minecraft:the_nether");
    let overworld_sky = sky_key(&app).unwrap();
    assert_eq!(app.world().resource::<ColumnStore>().len(), 2);

    receive_respawn(
        &mut app,
        connection,
        "minecraft:the_nether",
        nether.number(),
    );
    app.world_mut()
        .entity_mut(connection)
        .insert(current("minecraft:the_nether", nether));
    app.update();

    let store = app.world().resource::<ColumnStore>();
    assert!(
        store.is_empty(),
        "the overworld's columns are not the nether's"
    );
    assert_eq!(store.extent().map(|extent| extent.sections), Some(16));
    assert_ne!(
        sky_key(&app).unwrap(),
        overworld_sky,
        "the nether is drawn under its own sky"
    );

    app.world_mut().resource_mut::<ColumnStore>().insert(
        ColumnPos::new(3, 3),
        column(Extent {
            min_section_y: 0,
            sections: 16,
        }),
    );
    receive_respawn(
        &mut app,
        connection,
        "minecraft:the_nether",
        nether.number(),
    );
    assert_eq!(
        app.world().resource::<ColumnStore>().len(),
        1,
        "a respawn into the same dimension keeps its columns"
    );
}

#[test]
fn clearing_twice_or_before_play_is_harmless() {
    let mut app = play_app();
    app.update();
    let changed = app.world().resource_ref::<ColumnStore>().last_changed();

    let connection = app.world_mut().spawn(ConnectionState::Configuration).id();
    app.update();
    set_connection_state(&mut app, connection, ConnectionState::Configuration);
    set_connection_state(&mut app, connection, ConnectionState::Configuration);

    let world = app.world();
    assert!(world.resource::<ColumnStore>().is_empty());
    assert_eq!(
        world.resource_ref::<ColumnStore>().last_changed(),
        changed,
        "a client holding no columns writes none of its state"
    );
    assert!(world.get_resource::<Items>().is_none());
    assert!(world.get_resource::<ClockTimeMarkers>().is_none());
    assert!(world.get_resource::<SkyEnvironment>().is_none());
    assert!(world.resource::<WorldClocks>().is_empty());

    let registries = session_registries(&app, |_| {});
    app.insert_resource(registries);
    app.update();
    assert!(
        app.world().get_resource::<Items>().is_some(),
        "the tables are built once a set exists"
    );
}

#[test]
fn the_session_registries_rebuild_the_item_table() {
    let mut app = boot(|_| {});
    app.update();
    assert!(app.world().get_resource::<Items>().is_none());

    let first = session_registries(&app, |_| {});
    app.insert_resource(first);
    app.update();
    let songs = |app: &App| {
        app.world()
            .resource::<RegistrySet>()
            .registry::<JukeboxSong>()
            .expect("the jukebox songs are received")
    };
    let thirteen = songs(&app).require_by_name("minecraft:13").unwrap();
    assert_eq!(disc_song(app.world().resource::<Items>()), thirteen);

    let second = session_registries(&app, reversed("minecraft:jukebox_song"));
    app.insert_resource(second);
    app.update();
    let moved = songs(&app).require_by_name("minecraft:13").unwrap();
    assert_ne!(moved, thirteen, "the reordered set numbers the song anew");
    assert_eq!(
        disc_song(app.world().resource::<Items>()),
        moved,
        "the item table follows the set that replaced the first"
    );
}

#[test]
fn the_clock_markers_follow_the_session_set() {
    let mut app = boot(|_| {});
    app.update();
    assert!(app.world().get_resource::<ClockTimeMarkers>().is_none());
    assert!(app.world().resource::<WorldClocks>().is_empty());

    let clock_id = |app: &App, name: &str| {
        app.world()
            .resource::<RegistrySet>()
            .registry::<WorldClock>()
            .unwrap()
            .require_by_name(name)
            .unwrap()
    };
    let first = session_registries(&app, |_| {});
    let clock_count = first.registry::<WorldClock>().unwrap().len();
    app.insert_resource(first);
    app.update();
    let overworld = clock_id(&app, "minecraft:overworld");
    let markers = markers_by_clock(&app);
    assert!(
        markers.values().any(|count| *count > 0),
        "the timelines declare time markers"
    );
    assert_eq!(app.world().resource::<WorldClocks>().len(), clock_count);

    let second = session_registries(&app, reversed("minecraft:world_clock"));
    app.insert_resource(second);
    app.update();
    assert_ne!(
        clock_id(&app, "minecraft:overworld"),
        overworld,
        "the reordered set numbers the clock anew"
    );
    assert_eq!(
        markers_by_clock(&app),
        markers,
        "each clock keeps its markers under its new number"
    );
    assert_eq!(app.world().resource::<WorldClocks>().len(), clock_count);
}
