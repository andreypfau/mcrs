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
use mcrs_minecraft_command::{CommandLevel, Lookup, Source};
use mcrs_minecraft_command_argument::TimeLookup;
use mcrs_minecraft_command_argument_block::BlockLookup;
use mcrs_minecraft_command_argument_vanilla::ClientGraph;
use mcrs_minecraft_command_registry::RegistryLookup;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems, ConnectionEnded};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt, WritePacket};
use mcrs_minecraft_protocol_game::player::MessageSignature;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundChat, ServerboundChatCommand, ServerboundChatCommandSigned,
    ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol_game::text::Text;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocation};

use crate::GuiTick;
use crate::chat_history::{self, CommandHistoryFile};
use crate::chat_packets;
use crate::game_mode::PermissionLevel;
use crate::player::Player;
use crate::registries::SessionSet;
use crate::session::session_boundary;

#[derive(Message, Clone, Debug)]
pub struct SendChat(pub Outgoing);

#[derive(Message, Clone, Debug)]
pub struct ChatMessage {
    pub text: Text,
    pub tag: Option<Tag>,
    pub origin: MessageOrigin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageOrigin {
    System,
    Player,
}

impl ChatMessage {
    pub fn system(text: Text) -> Self {
        Self {
            text,
            tag: None,
            origin: MessageOrigin::System,
        }
    }
}

#[derive(Resource, Default)]
pub struct CommandTree(pub ClientGraph);

#[derive(Clone, Debug)]
pub struct OnlinePlayer {
    pub uuid: Uuid,
    pub name: String,
}

#[derive(Resource, Default, Debug)]
pub struct OnlinePlayers(pub Vec<OnlinePlayer>);

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

#[derive(Resource, Default)]
pub struct Chat {
    pub log: ChatLog<Text>,
    pub overlay: Overlay<Text>,
    pub history: History,
    pub draft: Option<String>,
    pub completions: Vec<String>,
    pub abilities: ChatAbilities,
    pub options: ChatOptions,
    pub(crate) player_chats: i32,
}

#[derive(Resource, Default)]
pub struct ChatInput(pub ChatScreen);

pub struct ClientLookup<'a> {
    registries: RegistryLookup<'a>,
    players: &'a OnlinePlayers,
    dimensions: &'a [ResourceKeyBuf<Dimension>],
}

impl Lookup for ClientLookup<'_> {
    fn entries(&self, registry: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        self.registries.entries(registry)
    }

    fn tags(&self, registry: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        self.registries.tags(registry)
    }

    fn has_registry(&self, registry: ResourceLocation<'static>) -> bool {
        self.registries.has_registry(registry)
    }

    fn tag_contains(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
        entry: ResourceLocation<'static>,
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
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
    ) -> impl Iterator<Item = &str> {
        self.registries.tag_entries(registry, tag)
    }
}

impl BlockLookup for ClientLookup<'_> {
    fn block_properties(&self, block: ResourceLocation<'_>) -> Option<&[BlockProperty]> {
        self.registries.block_properties(block)
    }

    fn has_block_entity(&self, block: ResourceLocation<'_>) -> bool {
        self.registries.has_block_entity(block)
    }
}

impl TimeLookup for ClientLookup<'_> {
    fn suggested_time_markers(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str> {
        self.registries.suggested_time_markers(clock)
    }

    fn timelines_of_clock(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str> {
        self.registries.timelines_of_clock(clock)
    }
}

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
    pub fn lookup(&self) -> ClientLookup<'_> {
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

    pub fn has_joined(&self) -> bool {
        !self.joined.is_empty()
    }

    pub fn level(&self) -> CommandLevel {
        CommandLevel::from(self.level.iter().next().map_or(0, |level| level.0))
    }
}

pub struct ClientChatPlugin {
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
            .add_message::<ConnectionEnded>()
            .add_systems(
                Update,
                clear_session_chat
                    .in_set(SessionSet::Clear)
                    .run_if(session_boundary),
            )
            .add_systems(
                Update,
                chat_history::finish_history_write.run_if(chat_history::history_writing),
            )
            .add_systems(
                Update,
                (
                    log_messages
                        .before(SessionSet::Clear)
                        .run_if(on_message::<ChatMessage>),
                    send_chat.run_if(on_message::<SendChat>),
                    request_suggestions.run_if(resource_changed::<ChatInput>),
                )
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush),
            )
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

fn clear_session_chat(
    mut chat: ResMut<Chat>,
    mut players: ResMut<OnlinePlayers>,
    mut tree: ResMut<CommandTree>,
    mut input: ResMut<ChatInput>,
) {
    chat.log.clear();
    chat.history.reset();
    chat.completions.clear();
    if chat.player_chats != 0 {
        chat.player_chats = 0;
    }
    players.0.clear();
    *tree = CommandTree::default();
    *input = ChatInput::default();
}

pub fn log_messages(
    mut messages: MessageReader<ChatMessage>,
    mut chat: ResMut<Chat>,
    tick: Res<GuiTick>,
) {
    let chat = &mut *chat;
    for message in messages.read() {
        let allowed = match message.origin {
            MessageOrigin::Player => chat.abilities.receive_player_messages,
            MessageOrigin::System => chat.abilities.receive_system_messages,
        };
        if allowed {
            chat.log.push(message.text.clone(), tick.0, message.tag);
        }
    }
}

pub fn context<'a>(
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

const NOTHING_SEEN: MessageSignature = MessageSignature {
    offset: VarInt(0),
    acknowledged: [0; 3],
    checksum: 1,
};

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the chat packet carries the wall-clock timestamp in milliseconds that the protocol requires"
    )
)]
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
