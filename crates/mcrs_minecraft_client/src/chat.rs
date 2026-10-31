//! The chat of the client: the log, the history, the open chat screen and the command parse
//! against what the client knows, and the packets that send a message or a command.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;
#[cfg(not(target_family = "wasm"))]
use std::time::{SystemTime, UNIX_EPOCH};

use crate::joined_game::{CurrentDimension, JoinedGame};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_chat_client::{
    ChatAbilities, ChatLog, ChatScreen, Context, History, Outgoing, Overlay, Signing, Tag,
};
use mcrs_minecraft_command::{CommandLevel, Identifier, Lookup, Source};
use mcrs_minecraft_command_argument::TimeLookup;
use mcrs_minecraft_command_argument_block::BlockLookup;
use mcrs_minecraft_command_argument_vanilla::ClientGraph;
use mcrs_minecraft_command_registry::RegistryLookup;
use mcrs_minecraft_core::{ResourceKey, StaticResourceLocation};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems};
use mcrs_minecraft_protocol::entity::player::MessageSignature;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChat, ServerboundChatCommand, ServerboundChatCommandSigned,
    ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt, WritePacket};
use mcrs_minecraft_registries::CurrentRegistries;

use crate::chat_history::{self, CommandHistoryFile};
use crate::chat_packets;
use crate::game_mode::PermissionLevel;
use crate::player::Player;

/// A line the player sent from the chat screen, for the plugin to put on the connection.
#[derive(Message, Clone, Debug)]
pub struct SendChat(pub Outgoing);

/// A message for the chat log. Debug feedback is one too.
#[derive(Message, Clone, Debug)]
pub struct ChatMessage {
    pub text: Text,
    pub tag: Option<Tag>,
    pub origin: MessageOrigin,
}

/// Which "receive messages" ability a message needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageOrigin {
    System,
    Player,
}

impl ChatMessage {
    /// A message from the server, not from a player.
    pub fn system(text: Text) -> Self {
        Self {
            text,
            tag: None,
            origin: MessageOrigin::System,
        }
    }
}

/// The client tree of the last `commands` packet.
#[derive(Resource, Default)]
pub struct CommandTree(pub ClientGraph);

/// A player in the player list.
#[derive(Clone, Debug)]
pub struct OnlinePlayer {
    pub uuid: Uuid,
    pub name: String,
}

/// The players in the player list.
#[derive(Resource, Default, Debug)]
pub struct OnlinePlayers(pub Vec<OnlinePlayer>);

/// The chat options that the client reads; the client has no options screen.
#[derive(Clone, Copy, Debug)]
pub struct ChatOptions {
    pub auto_suggest: bool,
    pub save_unsent: bool,
    pub text_background_opacity: f32,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            auto_suggest: true,
            save_unsent: false,
            text_background_opacity: 0.5,
        }
    }
}

/// The chat state that lives across screens: the log, the overlay message, the history, the
/// draft and the custom completion set.
#[derive(Resource, Default)]
pub struct Chat {
    pub log: ChatLog<Text>,
    pub overlay: Overlay<Text>,
    pub history: History,
    pub draft: Option<String>,
    pub completions: Vec<String>,
    pub abilities: ChatAbilities,
    pub options: ChatOptions,
    /// The GUI tick: client ticks while the game runs.
    pub tick: u64,
    pub(crate) player_chats: i32,
}

/// The chat screen while it is open, kept between opens so its buffers stay.
#[derive(Resource, Default)]
pub struct ChatInput(pub ChatScreen);

/// What the client knows that a command argument reads.
pub struct ClientLookup<'a> {
    registries: RegistryLookup<'a>,
    players: &'a OnlinePlayers,
    dimensions: &'a [ResourceKey<Dimension>],
}

impl Lookup for ClientLookup<'_> {
    fn entries(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.registries.entries(registry)
    }

    fn tags(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.registries.tags(registry)
    }

    fn has_registry(&self, registry: StaticResourceLocation) -> bool {
        self.registries.has_registry(registry)
    }

    fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool {
        self.registries.tag_contains(registry, tag, entry)
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        self.players.0.iter().map(|player| player.name.as_str())
    }

    fn dimensions(&self) -> impl Iterator<Item = &str> {
        self.dimensions.iter().map(|key| key.as_str())
    }

    fn tag_entries(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> impl Iterator<Item = &str> {
        self.registries.tag_entries(registry, tag)
    }
}

impl BlockLookup for ClientLookup<'_> {
    fn block_properties(&self, block: Identifier<'_>) -> Option<&[BlockProperty]> {
        self.registries.block_properties(block)
    }

    fn has_block_entity(&self, block: Identifier<'_>) -> bool {
        self.registries.has_block_entity(block)
    }
}

impl TimeLookup for ClientLookup<'_> {
    fn suggested_time_markers(&self, clock: Option<Identifier<'_>>) -> impl Iterator<Item = &str> {
        self.registries.suggested_time_markers(clock)
    }

    fn timelines_of_clock(&self, clock: Option<Identifier<'_>>) -> impl Iterator<Item = &str> {
        self.registries.timelines_of_clock(clock)
    }
}

/// The world data a command parse reads, gathered for one system.
#[derive(SystemParam)]
pub struct CommandWorld<'w, 's> {
    tree: Res<'w, CommandTree>,
    registries: Option<Res<'w, CurrentRegistries>>,
    blocks: Option<Res<'w, Blocks>>,
    players: Res<'w, OnlinePlayers>,
    joined: Query<'w, 's, &'static JoinedGame>,
    level: Query<'w, 's, &'static PermissionLevel, With<Player>>,
    dimension: Query<'w, 's, &'static CurrentDimension>,
}

impl CommandWorld<'_, '_> {
    pub(crate) fn lookup(&self) -> ClientLookup<'_> {
        ClientLookup {
            registries: RegistryLookup {
                registries: self.registries.as_deref().map(|registries| &**registries),
                blocks: self.blocks.as_deref().map(|blocks| &*blocks.0),
                dimension_type: self
                    .dimension
                    .iter()
                    .next()
                    .map(|current| current.dimension_type),
            },
            players: &self.players,
            dimensions: self
                .joined
                .iter()
                .next()
                .map_or(&[], |joined| joined.dimensions.as_slice()),
        }
    }

    pub(crate) fn has_joined(&self) -> bool {
        !self.joined.is_empty()
    }

    pub(crate) fn level(&self) -> CommandLevel {
        CommandLevel::from(self.level.iter().next().map_or(0, |level| level.0))
    }
}

/// The chat of the client, with the command history kept in `command_history`.
pub struct ClientChatPlugin {
    /// Where the command history lives, if anywhere.
    pub command_history: Option<PathBuf>,
}

impl Plugin for ClientChatPlugin {
    fn build(&self, app: &mut App) {
        let mut chat = Chat::default();
        if let Some(path) = &self.command_history {
            if let Ok(file) = std::fs::read_to_string(path) {
                chat.history = History::from_command_file(&file);
            }
            app.insert_resource(CommandHistoryFile::new(path.clone()));
        }
        app.insert_resource(chat)
            .init_resource::<CommandTree>()
            .init_resource::<OnlinePlayers>()
            .init_resource::<ChatInput>()
            .add_message::<ChatMessage>()
            .add_message::<SendChat>()
            .add_systems(FixedUpdate, advance_tick)
            .add_systems(
                Update,
                chat_history::finish_history_write.run_if(chat_history::history_writing),
            )
            .add_systems(
                Update,
                (log_messages, send_chat, request_suggestions)
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush),
            )
            .add_observer(chat_packets::on_login)
            .add_observer(chat_packets::on_commands)
            .add_observer(chat_packets::on_suggestions)
            .add_observer(chat_packets::on_system_chat)
            .add_observer(chat_packets::on_player_chat)
            .add_observer(chat_packets::on_disguised_chat)
            .add_observer(chat_packets::on_action_bar)
            .add_observer(chat_packets::on_custom_completions)
            .add_observer(chat_packets::on_player_info_update)
            .add_observer(chat_packets::on_player_info_remove);
    }
}

fn advance_tick(mut chat: ResMut<Chat>) {
    chat.tick = chat.tick.saturating_add(1);
    chat.overlay.tick();
}

pub(crate) fn log_messages(mut messages: MessageReader<ChatMessage>, mut chat: ResMut<Chat>) {
    let chat = &mut *chat;
    for message in messages.read() {
        let allowed = match message.origin {
            MessageOrigin::Player => chat.abilities.receive_player_messages,
            MessageOrigin::System => chat.abilities.receive_system_messages,
        };
        if allowed {
            chat.log.push(message.text.clone(), chat.tick, message.tag);
        }
    }
}

pub(crate) fn context<'a>(
    world: &'a CommandWorld,
    source: &'a Source<'a, ClientLookup<'a>>,
    abilities: ChatAbilities,
    options: ChatOptions,
    completions: &'a [String],
) -> Context<'a, ClientLookup<'a>> {
    Context {
        tree: &world.tree.0,
        source,
        abilities,
        auto_suggest: options.auto_suggest,
        completions,
    }
}

fn now_millis() -> u64 {
    #[cfg(not(target_family = "wasm"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64)
    }
    #[cfg(target_family = "wasm")]
    {
        0
    }
}

fn salt() -> u64 {
    RandomState::new().build_hasher().finish()
}

/// An unsigned session acknowledges nothing.
const NOTHING_SEEN: MessageSignature = MessageSignature {
    offset: VarInt(0),
    acknowledged: [0; 3],
    checksum: 1,
};

fn send_chat(mut sent: MessageReader<SendChat>, connection: Option<Single<&mut ClientConnection>>) {
    let Some(mut connection) = connection else {
        sent.clear();
        return;
    };
    for SendChat(outgoing) in sent.read() {
        send(&mut connection, outgoing);
    }
}

fn request_suggestions(
    mut input: ResMut<ChatInput>,
    connection: Option<Single<&mut ClientConnection>>,
) {
    let screen = input.bypass_change_detection();
    let Some(request) = screen.0.take_request() else {
        return;
    };
    if let Some(mut connection) = connection {
        connection.write_packet(&ServerboundCommandSuggestion {
            id: VarInt(request.id),
            text: Bounded(screen.0.input().get(..request.end).unwrap_or_default()),
        });
    }
}

fn send(connection: &mut ClientConnection, send: &Outgoing) {
    match send {
        Outgoing::Chat(message) => connection.write_packet(&ServerboundChat {
            message: Bounded(message),
            timestamp: now_millis(),
            salt: salt(),
            signature: None,
            last_seen_messages: NOTHING_SEEN,
        }),
        Outgoing::Command {
            line,
            signing: Signing::Unsigned,
        } => connection.write_packet(&ServerboundChatCommand {
            command: Bounded(line),
        }),
        Outgoing::Command {
            line,
            signing: Signing::Signed,
        } => {
            connection.write_packet(&ServerboundChatCommandSigned {
                command: Bounded(line),
                timestamp: now_millis(),
                salt: salt(),
                argument_signatures: Vec::new(),
                last_seen_messages: NOTHING_SEEN,
            });
        }
    }
}
