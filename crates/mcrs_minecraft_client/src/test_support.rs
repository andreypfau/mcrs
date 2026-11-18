use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::time::Duration;

use bevy::platform::cell::SyncCell;
use bevy::prelude::*;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::session::LocalWorldOrigin;
use mcrs_minecraft_client_world::session::OpenLocalWorld;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_stack::write_wire_with_length;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_network::client::ClientSession;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::Encode;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetPlayerInventory;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol_game::item::DelimitedStack;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocation;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServerAnswers {
    pub slots: u32,
    pub cursors: u32,
    pub contents: u32,
    pub player_slots: u32,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatFeedback(pub u32);

pub fn count_server_answers(app: &mut App) {
    app.init_resource::<ServerAnswers>()
        .init_resource::<ChatFeedback>()
        .add_observer(
            |_: On<Received<ClientboundContainerSetSlot>>, mut answers: ResMut<ServerAnswers>| {
                answers.slots += 1;
            },
        )
        .add_observer(
            |_: On<Received<ClientboundSetCursorItem>>, mut answers: ResMut<ServerAnswers>| {
                answers.cursors += 1;
            },
        )
        .add_observer(
            |_: On<Received<ClientboundContainerSetContent>>,
             mut answers: ResMut<ServerAnswers>| {
                answers.contents += 1;
            },
        )
        .add_observer(
            |_: On<Received<ClientboundSetPlayerInventory>>, mut answers: ResMut<ServerAnswers>| {
                answers.player_slots += 1;
            },
        )
        .add_observer(
            |_: On<Received<ClientboundSystemChatPacket<'static>>>,
             mut feedback: ResMut<ChatFeedback>| {
                feedback.0 += 1;
            },
        );
}

#[derive(Resource)]
pub struct CreativeSlotTap(Sender<ServerboundSetCreativeModeSlot>);

#[derive(Resource)]
struct CreativeSlotReceiver(SyncCell<Receiver<ServerboundSetCreativeModeSlot>>);

#[derive(Clone, Debug, PartialEq)]
pub struct ReceivedCreativeSlot {
    pub slot: i16,
    pub stack: Option<ItemStack>,
}

pub fn record_creative_slots(app: &mut App) {
    let (sender, receiver) = mpsc::channel();
    app.insert_resource(CreativeSlotTap(sender))
        .insert_resource(CreativeSlotReceiver(SyncCell::new(receiver)));
}

pub fn received_creative_slots(world: &mut World) -> Vec<ReceivedCreativeSlot> {
    let packets: Vec<ServerboundSetCreativeModeSlot> = world
        .resource_mut::<CreativeSlotReceiver>()
        .0
        .get()
        .try_iter()
        .collect();
    let names = RegistryNames::new(
        world.resource::<CurrentRegistries>(),
        world.resource::<ItemTable>(),
    );
    packets
        .into_iter()
        .map(|packet| {
            let mut bytes = Vec::new();
            packet.item.encode(&mut bytes).unwrap();
            ReceivedCreativeSlot {
                slot: packet.slot,
                stack: read_wire_with_length(&mut bytes.as_slice(), &names).unwrap(),
            }
        })
        .collect()
}

pub(crate) fn tap_server<E>(
    build: impl FnOnce() -> Result<App, E> + Send + 'static,
    tap: Option<&CreativeSlotTap>,
) -> impl FnOnce() -> Result<App, E> + Send + 'static {
    let sender = tap.map(|tap| tap.0.clone());
    move || {
        let mut app = build()?;
        if let Some(sender) = sender {
            app.add_observer(
                move |received: On<Received<ServerboundSetCreativeModeSlot>>| {
                    if let Some(packet) = received.packet() {
                        sender.send(packet).ok();
                    }
                },
            );
        }
        Ok(app)
    }
}

pub fn open(app: &mut App, folder: &Path) {
    app.world_mut().write_message(OpenLocalWorld {
        folder: folder.to_owned(),
        host: None,
        origin: LocalWorldOrigin::Opened,
        lock: None,
    });
}

pub fn update_until(app: &mut App, mut done: impl FnMut(&mut App) -> bool) {
    while !done(app) {
        std::thread::sleep(Duration::from_millis(5));
        app.update();
    }
}

pub fn joined(app: &mut App) -> bool {
    app.world_mut()
        .query_filtered::<(), (With<ClientSession>, With<JoinedGame>)>()
        .iter(app.world())
        .next()
        .is_some()
}

pub fn creative_player(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<(Entity, &PlayerGameMode), (With<PlayerHolders>, With<Slots>)>()
        .iter(world)
        .find(|(_, game_mode)| game_mode.0 == GameMode::Creative)
        .map(|(player, _)| player)
}

pub fn round_trip(app: &mut App) {
    for _ in 0..2 {
        let seen = app.world().resource::<ChatFeedback>().0;
        connection(app.world_mut()).write_packet(&ServerboundChatCommand {
            command: Bounded("time query gametime"),
        });
        update_until(app, |app| app.world().resource::<ChatFeedback>().0 > seen);
    }
}

pub fn connection(world: &mut World) -> Mut<'_, ClientConnection> {
    world
        .query::<&mut ClientConnection>()
        .single_mut(world)
        .expect("one connection")
}

#[must_use]
pub fn player_slot(entry: u16) -> u16 {
    match entry {
        9..=35 => entry,
        36..=44 => entry - 36,
        45 => 40,
        _ => panic!("only the entries 9 to 45 show a player slot"),
    }
}

#[must_use]
pub fn item_kind(world: &World, name: &str) -> Kind {
    let location = ResourceLocation::read(name).unwrap();
    Kind(
        world
            .resource::<ItemTable>()
            .id(NameTable::Item, location)
            .unwrap(),
    )
}

pub fn give(world: &mut World, viewer: Entity, entry: u16, value: Option<ItemStack>) {
    let names = RegistryNames::new(
        world.resource::<CurrentRegistries>(),
        world.resource::<ItemTable>(),
    );
    let mut bytes = Vec::new();
    write_wire_with_length(value.as_ref(), &names, &mut bytes).unwrap();
    let item = DelimitedStack::decode(&mut bytes.as_slice()).unwrap();
    connection(world).write_packet(&ServerboundSetCreativeModeSlot {
        slot: i16::try_from(entry).unwrap(),
        item,
    });
    put_local(world, viewer, entry, value);
}

pub fn put_local(world: &mut World, viewer: Entity, entry: u16, value: Option<ItemStack>) {
    let index = player_slot(entry);
    let occupant = world
        .get::<Slots>(viewer)
        .and_then(|slots| slots.get(usize::from(index)));
    let mut commands = world.commands();
    if let Some(occupant) = occupant {
        commands.entity(occupant).despawn();
    }
    if let Some(value) = value {
        commands.spawn_stack(value).put_at(viewer, index);
    }
    world.flush();
}
