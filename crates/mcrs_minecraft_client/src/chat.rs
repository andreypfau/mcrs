use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_command::GraphBuilder;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_chat::{
    ChatAbilities, ChatLog, ChatScreen, Closed, Context, History, Key, Outcome, Overlay, Request,
    Send, Tag, keeps_draft,
};
use mcrs_minecraft_command::{ClientGraph, CommandLevel, Identifier, Lookup, Source, client_tree};
use mcrs_minecraft_core::{ResourceLocation, StaticResourceLocation};
use mcrs_minecraft_dimension::keys::DIMENSION;
use mcrs_minecraft_network::client::{
    ClientConnection, ClientNetworkSystems, JoinedGame, close_connection_later,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::chat::CompletionAction;
use mcrs_minecraft_protocol::entity::player::MessageSignature;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundCommandSuggestions, ClientboundCommands, ClientboundCustomChatCompletions,
    ClientboundDisguisedChat, ClientboundLogin, ClientboundPlayerChat, ClientboundPlayerInfoRemove,
    ClientboundPlayerInfoUpdate, ClientboundSetActionBarText, ClientboundSystemChatPacket,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChat, ServerboundChatCommand, ServerboundChatCommandSigned,
    ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt, WritePacket};
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_world::chat_type::{ChatParameter, ChatType};

use crate::game_mode::PermissionLevel;
use crate::gui::chat::{list_rows, per_page, row_at};
use crate::gui::scene::{GuiAtlas, GuiScene};
use crate::player::Player;
use crate::screen::{Screen, grab};

/// The registries whose identifiers the client can name, so their arguments parse on the
/// client.
// chisle: a fixed list; an argument of another registry reads as a type without a grammar and
// asks the server. Registry keys that are not static lift it.
const STATIC_REGISTRIES: [StaticResourceLocation; 3] = [
    mcrs_minecraft_biome::keys::BIOME.location(),
    mcrs_minecraft_entity::keys::STRUCTURE.location(),
    mcrs_minecraft_keys::POINT_OF_INTEREST_TYPE,
];

/// A message for the chat log (CHAT-RECV-001 to CHAT-RECV-003). Debug feedback is one too.
#[derive(Message, Clone, Debug)]
pub struct ChatMessage {
    pub text: Text,
    pub tag: Option<Tag>,
    /// The source is a player, so it needs "receive player messages" (CHAT-OPT-002).
    pub from_player: bool,
}

impl ChatMessage {
    pub fn system(text: Text) -> Self {
        Self {
            text,
            tag: None,
            from_player: false,
        }
    }
}

/// The client tree of the last `commands` packet (CCMD-TREE-001).
#[derive(Resource)]
pub struct CommandTree(pub ClientGraph);

impl Default for CommandTree {
    fn default() -> Self {
        Self(
            GraphBuilder::default()
                .freeze()
                .expect("a tree of one root freezes"),
        )
    }
}

/// The names of the players in the player list.
#[derive(Resource, Default, Debug)]
pub struct OnlinePlayers(pub Vec<(Uuid, String)>);

/// The options of CHAT-OPT-001 that the client reads; the client has no options screen.
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
    pub log: ChatLog<Text, Vec<crate::gui::font::Span>>,
    pub overlay: Overlay<Text>,
    pub history: History,
    pub draft: Option<String>,
    pub completions: Vec<String>,
    pub abilities: ChatAbilities,
    pub options: ChatOptions,
    /// The GUI tick: client ticks while the game runs.
    pub tick: u64,
    player_chats: i32,
}

/// The chat screen while it is open, kept between opens so its buffers stay.
#[derive(Resource, Default)]
pub struct ChatInput(pub ChatScreen);

/// What the client knows that a command argument reads (CCMD-TREE-001, CCMD-SUG-002).
pub struct ClientLookup<'a> {
    registries: Option<&'a Registries>,
    blocks: Option<&'a Blocks>,
    players: &'a OnlinePlayers,
    dimensions: &'a [mcrs_minecraft_core::ResourceKey<mcrs_minecraft_dimension::Dimension>],
}

impl ClientLookup<'_> {
    fn names(&self, registry: StaticResourceLocation) -> &[ResourceLocation] {
        self.registries
            .and_then(|registries| registries.names_of(&registry))
            .unwrap_or_default()
    }

    fn tag_members(&self, registry: StaticResourceLocation, tag: Identifier<'_>) -> &[u16] {
        self.registries
            .and_then(|registries| registries.tags_of(&registry))
            .and_then(|mut tags| tags.find(|(name, _)| tag.eq_str(name.as_str())))
            .map_or(&[], |(_, members)| members)
    }
}

impl Lookup for ClientLookup<'_> {
    fn entries(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        let dimensions = if registry == DIMENSION.location() {
            self.dimensions
        } else {
            &[]
        };
        dimensions
            .iter()
            .map(|key| key.as_str())
            .chain(self.names(registry).iter().map(ResourceLocation::as_str))
    }

    fn tags(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.registries
            .and_then(|registries| registries.tags_of(&registry))
            .into_iter()
            .flatten()
            .map(|(name, _)| name.as_str())
    }

    fn has_registry(&self, registry: StaticResourceLocation) -> bool {
        registry == DIMENSION.location()
            || self
                .registries
                .is_some_and(|registries| registries.names_of(&registry).is_some())
    }

    fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool {
        self.registries
            .and_then(|registries| registries.id_of(&registry, entry.as_str()))
            .is_some_and(|id| self.tag_members(registry, tag).contains(&id))
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        self.players.0.iter().map(|(_, name)| name.as_str())
    }

    fn tag_entries(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> impl Iterator<Item = &str> {
        let names = self.names(registry);
        self.tag_members(registry, tag)
            .iter()
            .filter_map(move |&id| names.get(usize::from(id)))
            .map(ResourceLocation::as_str)
    }

    // chisle: a linear scan of the block table per read; an index by identifier lifts it.
    fn block_properties(&self, block: Identifier<'_>) -> Option<&[BlockProperty]> {
        self.blocks?
            .blocks()
            .iter()
            .find(|entry| block.eq_str(entry.identifier.as_str()))
            .map(|entry| entry.properties.0.as_slice())
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
}

impl CommandWorld<'_, '_> {
    fn lookup(&self) -> ClientLookup<'_> {
        ClientLookup {
            registries: self.registries.as_deref().map(|registries| &**registries),
            blocks: self.blocks.as_deref(),
            players: &self.players,
            dimensions: self
                .joined
                .iter()
                .next()
                .map_or(&[], |joined| joined.dimensions.as_slice()),
        }
    }

    fn level(&self) -> CommandLevel {
        CommandLevel::from_id(self.level.iter().next().map_or(0, |level| level.0))
    }
}

pub struct ChatPlugin {
    /// Where the command history lives, if anywhere (CHAT-HIST-001).
    pub command_history: Option<PathBuf>,
}

#[derive(Resource)]
struct CommandHistoryFile(PathBuf);

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        let mut chat = Chat::default();
        if let Some(path) = &self.command_history {
            if let Ok(file) = std::fs::read_to_string(path) {
                chat.history = History::from_command_file(&file);
            }
            app.insert_resource(CommandHistoryFile(path.clone()));
        }
        app.insert_resource(chat)
            .init_resource::<CommandTree>()
            .init_resource::<OnlinePlayers>()
            .init_resource::<ChatInput>()
            .add_message::<ChatMessage>()
            .add_systems(FixedUpdate, advance_tick)
            .add_systems(PreUpdate, chat_keys.after(InputSystems))
            .add_systems(
                Update,
                log_messages
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush),
            )
            .add_observer(on_login)
            .add_observer(on_commands)
            .add_observer(on_suggestions)
            .add_observer(on_system_chat)
            .add_observer(on_player_chat)
            .add_observer(on_disguised_chat)
            .add_observer(on_action_bar)
            .add_observer(on_custom_completions)
            .add_observer(on_player_info_update)
            .add_observer(on_player_info_remove);
    }
}

fn advance_tick(mut chat: ResMut<Chat>) {
    chat.tick += 1;
    chat.overlay.tick();
}

fn log_messages(
    mut messages: MessageReader<ChatMessage>,
    mut chat: ResMut<Chat>,
    screen: Res<Screen>,
    language: Option<Res<crate::gui::language::Language>>,
    atlas: Option<Res<GuiAtlas>>,
) {
    let open = *screen == Screen::Chat;
    let chat = &mut *chat;
    for message in messages.read() {
        let allowed = if message.from_player {
            chat.abilities.receive_player_messages
        } else {
            chat.abilities.receive_system_messages
        };
        if !allowed {
            continue;
        }
        let width = crate::gui::chat::wrap_width(message.tag);
        chat.log.push(
            message.text.clone(),
            chat.tick,
            message.tag,
            open,
            |text, rows| {
                let Some(language) = language.as_deref() else {
                    return;
                };
                let spans = crate::gui::text::text_spans(text, language, 0xFFFF_FFFF);
                match atlas.as_deref() {
                    Some(atlas) => crate::gui::text::wrap(&spans, width, true, &atlas.0.font, rows),
                    None => rows.push(spans),
                }
            },
        );
    }
}

fn on_login(
    _: On<Received<ClientboundLogin>>,
    mut chat: ResMut<Chat>,
    mut players: ResMut<OnlinePlayers>,
) {
    chat.log.clear();
    chat.history.reset();
    chat.completions.clear();
    chat.player_chats = 0;
    players.0.clear();
}

fn on_commands(
    event: On<Received<ClientboundCommands<'static>>>,
    mut tree: ResMut<CommandTree>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    match client_tree(&packet, &STATIC_REGISTRIES) {
        Ok(graph) => tree.0 = graph,
        Err(error) => close_connection_later(
            &mut commands,
            event.entity,
            format!("the command tree is invalid: {error}"),
        ),
    }
}

fn context<'a>(
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

fn on_suggestions(
    event: On<Received<ClientboundCommandSuggestions<'static>>>,
    world: CommandWorld,
    chat: Res<Chat>,
    mut input: ResMut<ChatInput>,
) {
    let Some(reply) = event.packet() else {
        return;
    };
    let lookup = world.lookup();
    let source = Source::new(world.level(), &lookup);
    let cx = context(
        &world,
        &source,
        chat.abilities,
        chat.options,
        &chat.completions,
    );
    let start = usize::try_from(reply.start.0).unwrap_or_default();
    let length = usize::try_from(reply.length.0).unwrap_or_default();
    input
        .0
        .commands
        .reply(reply.id.0, start, length, &reply.suggestions, &cx);
}

fn on_system_chat(
    event: On<Received<ClientboundSystemChatPacket>>,
    mut chat: ResMut<Chat>,
    mut messages: MessageWriter<ChatMessage>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if packet.overlay {
        chat.overlay.set(packet.content);
    } else {
        messages.write(ChatMessage {
            text: packet.content,
            tag: Some(Tag::SystemSinglePlayer),
            from_player: false,
        });
    }
}

fn on_action_bar(event: On<Received<ClientboundSetActionBarText>>, mut chat: ResMut<Chat>) {
    if let Some(packet) = event.packet() {
        chat.overlay.set(packet.text);
    }
}

/// The `chat` decoration of a chat type around a content (CHAT-RECV-004).
fn decorate(
    registries: Option<&Registries>,
    chat_type: u32,
    sender: &Text,
    target: Option<&Text>,
    content: Text,
) -> Text {
    let Some(decoration) = registries
        .and_then(|registries| registries.registry::<ChatType>())
        .and_then(|registry| registry.values().get(chat_type as usize).cloned())
        .map(|chat_type| chat_type.chat)
    else {
        return content;
    };
    let arguments = decoration
        .parameters
        .iter()
        .map(|parameter| match parameter {
            ChatParameter::Sender => sender.clone(),
            ChatParameter::Target => target.cloned().unwrap_or_default(),
            ChatParameter::Content => content.clone(),
        })
        .collect::<Vec<_>>();
    let mut text = Text::translate(decoration.translation_key, arguments);
    let style = &decoration.style;
    text.style.color = style.color;
    text.style.bold = style.bold;
    text.style.italic = style.italic;
    text.style.underlined = style.underlined;
    text.style.strikethrough = style.strikethrough;
    text.style.obfuscated = style.obfuscated;
    text
}

fn on_player_chat(
    event: On<Received<ClientboundPlayerChat<'static>>>,
    mut chat: ResMut<Chat>,
    players: Res<OnlinePlayers>,
    registries: Option<Res<CurrentRegistries>>,
    mut messages: MessageWriter<ChatMessage>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if packet.global_index.0 != chat.player_chats {
        close_connection_later(
            &mut commands,
            event.entity,
            "multiplayer.disconnect.bad_chat_index".to_owned(),
        );
        return;
    }
    chat.player_chats += 1;
    let registries = registries.as_deref().map(|registries| &**registries);
    let decorate = |content| {
        decorate(
            registries,
            packet.chat_type.0,
            &packet.sender_name,
            packet.target_name.as_ref(),
            content,
        )
    };
    if !players.0.iter().any(|(uuid, _)| *uuid == packet.sender) {
        messages.write(ChatMessage {
            text: decorate(
                Text::translate("chat.validation_error", vec![])
                    .color(Color::RED)
                    .italic(),
            ),
            tag: Some(Tag::Error),
            from_player: true,
        });
        return;
    }
    let content = match (&packet.filter, packet.unsigned_content) {
        (mcrs_minecraft_protocol::chat::FilterMask::FullyFiltered, _) => return,
        (mcrs_minecraft_protocol::chat::FilterMask::PartiallyFiltered(_), _) => {
            filtered(&packet.content, &packet.filter)
        }
        (_, Some(unsigned)) => unsigned,
        (_, None) => packet.content.0.to_owned().into_text(),
    };
    messages.write(ChatMessage {
        text: decorate(content),
        tag: packet.signature.is_none().then_some(Tag::NotSecure),
        from_player: true,
    });
}

/// The body with each hidden code unit as a dark grey `#` (CHAT-RECV-002, step 6).
fn filtered(body: &str, mask: &mcrs_minecraft_protocol::chat::FilterMask) -> Text {
    let mut text = Text::text("");
    let mut run = String::new();
    let mut hidden_run = false;
    let mut index = 0;
    let flush = |run: &mut String, hidden: bool, text: &mut Text| {
        if run.is_empty() {
            return;
        }
        let part = std::mem::take(run).into_text();
        let part = if hidden {
            part.color(Color::DARK_GRAY)
                .on_hover_show_text(Text::translate("chat.filtered", vec![]))
        } else {
            part
        };
        *text = std::mem::take(text).add_child(part);
    };
    for c in body.chars() {
        let hidden = mask.hides(index);
        index += c.len_utf16();
        if hidden != hidden_run {
            flush(&mut run, hidden_run, &mut text);
            hidden_run = hidden;
        }
        run.push(if hidden { '#' } else { c });
    }
    flush(&mut run, hidden_run, &mut text);
    text
}

fn on_disguised_chat(
    event: On<Received<ClientboundDisguisedChat>>,
    registries: Option<Res<CurrentRegistries>>,
    mut messages: MessageWriter<ChatMessage>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let registries = registries.as_deref().map(|registries| &**registries);
    messages.write(ChatMessage {
        text: decorate(
            registries,
            packet.chat_type.0,
            &packet.sender_name,
            packet.target_name.as_ref(),
            packet.message,
        ),
        tag: Some(Tag::System),
        from_player: true,
    });
}

fn on_custom_completions(
    event: On<Received<ClientboundCustomChatCompletions<'static>>>,
    mut chat: ResMut<Chat>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let completions = &mut chat.completions;
    match packet.action {
        CompletionAction::Set => {
            completions.clear();
            completions.extend(packet.entries.iter().map(|entry| entry.to_string()));
        }
        CompletionAction::Add => {
            for entry in &packet.entries {
                if !completions.iter().any(|known| known == entry) {
                    completions.push(entry.to_string());
                }
            }
        }
        CompletionAction::Remove => {
            completions.retain(|known| !packet.entries.iter().any(|entry| entry == known));
        }
    }
}

fn on_player_info_update(
    event: On<Received<ClientboundPlayerInfoUpdate<'static>>>,
    mut players: ResMut<OnlinePlayers>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !packet.actions.add_player() {
        return;
    }
    for entry in packet.entries.iter() {
        players.0.retain(|(uuid, _)| *uuid != entry.player_uuid);
        players
            .0
            .push((entry.player_uuid, entry.username.to_string()));
    }
}

fn on_player_info_remove(
    event: On<Received<ClientboundPlayerInfoRemove>>,
    mut players: ResMut<OnlinePlayers>,
) {
    if let Some(packet) = event.packet() {
        players.0.retain(|(uuid, _)| !packet.players.contains(uuid));
    }
}

fn now_millis() -> u64 {
    #[cfg(not(target_family = "wasm"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64)
    }
    #[cfg(target_family = "wasm")]
    {
        0
    }
}

fn salt() -> u64 {
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

/// An unsigned session acknowledges nothing (CHAT-SEND-005).
const NOTHING_SEEN: MessageSignature = MessageSignature {
    offset: VarInt(0),
    acknowledged: [0; 3],
    checksum: 1,
};

fn send(connection: &mut ClientConnection, send: &Send) {
    match send {
        Send::Chat(message) => connection.write_packet(&ServerboundChat {
            message: Bounded(message),
            timestamp: now_millis(),
            salt: salt(),
            signature: None,
            last_seen_messages: NOTHING_SEEN,
        }),
        Send::Command {
            line,
            signed: false,
        } => connection.write_packet(&ServerboundChatCommand {
            command: Bounded(line),
        }),
        Send::Command { line, signed: true } => {
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

fn send_request(connection: Option<&mut ClientConnection>, input: &str, request: Request) {
    if let Some(connection) = connection {
        connection.write_packet(&ServerboundCommandSuggestion {
            id: VarInt(request.id),
            text: Bounded(input.get(..request.end).unwrap_or_default()),
        });
    }
}

fn modifiers(keys: &ButtonInput<KeyCode>) -> (bool, bool) {
    let control = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    (control, shift)
}

/// The key of the chat screen, or `None` for a key that types text or does nothing.
fn screen_key(code: KeyCode, control: bool, shift: bool) -> Option<Key> {
    Some(match code {
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::Tab => Key::Tab { back: shift },
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Backspace => Key::Backspace { word: control },
        KeyCode::Delete => Key::Delete { word: control },
        KeyCode::ArrowLeft => Key::Left {
            word: control,
            select: shift,
        },
        KeyCode::ArrowRight => Key::Right {
            word: control,
            select: shift,
        },
        KeyCode::Home => Key::Home { select: shift },
        KeyCode::End => Key::End { select: shift },
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::KeyA if control => Key::SelectAll,
        KeyCode::KeyC if control => Key::Copy,
        KeyCode::KeyX if control => Key::Cut,
        _ => return None,
    })
}

#[derive(SystemParam)]
struct ChatKeys<'w, 's> {
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    scroll: Res<'w, AccumulatedMouseScroll>,
    clipboard: Option<ResMut<'w, bevy::clipboard::Clipboard>>,
    scene: Res<'w, GuiScene>,
    atlas: Option<Res<'w, GuiAtlas>>,
    last_cursor: Local<'s, IVec2>,
}

#[derive(SystemParam)]
struct ChatState<'w> {
    screen: ResMut<'w, Screen>,
    chat: ResMut<'w, Chat>,
    input: ResMut<'w, ChatInput>,
    history_file: Option<Res<'w, CommandHistoryFile>>,
}

/// The initial input of a key that opens the chat screen, and whether it is a draft
/// (CHAT-OPEN-001).
fn opening(
    keys: &ButtonInput<KeyCode>,
    screen: Screen,
    joined: bool,
    chat: &mut Chat,
) -> Option<(String, bool)> {
    if screen != Screen::None || !joined {
        return None;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        return Some(
            chat.draft
                .take()
                .map_or((String::new(), false), |draft| (draft, true)),
        );
    }
    if !keys.just_pressed(KeyCode::Slash) {
        return None;
    }
    Some(match chat.draft.take() {
        Some(draft) if draft.starts_with('/') => (draft, true),
        other => {
            chat.draft = other;
            ("/".to_owned(), false)
        }
    })
}

/// Opens the chat screen on its keys (CHAT-OPEN-001), and while it is open gives it every key,
/// so no game key fires under it (CHAT-KEY-001).
fn chat_keys(
    mut keys: ChatKeys,
    mut state: ChatState,
    world: CommandWorld,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut connection: Option<Single<&mut ClientConnection>>,
) {
    let lookup = world.lookup();
    let source = Source::new(world.level(), &lookup);
    let chat = &mut *state.chat;
    if *state.screen != Screen::Chat {
        let Some((initial, draft)) =
            opening(&keys.keys, *state.screen, !world.joined.is_empty(), chat)
        else {
            return;
        };
        let cx = context(
            &world,
            &source,
            chat.abilities,
            chat.options,
            &chat.completions,
        );
        state.input.0.open(&initial, draft, &cx);
        flush_request(&mut state.input.0, &mut connection);
        *state.screen = Screen::Chat;
        grab(&mut cursor, false);
        keys.keyboard.clear();
        keys.keys.reset_all();
        return;
    }
    let cx = context(
        &world,
        &source,
        chat.abilities,
        chat.options,
        &chat.completions,
    );
    let (control, shift) = modifiers(&keys.keys);
    let screen = &mut state.input.0;
    let mut closed = None;
    let mut page = 0;
    for event in keys.keyboard.read() {
        if event.state != ButtonState::Pressed || closed.is_some() {
            continue;
        }
        let outcome = if let Some(key) = screen_key(event.key_code, control, shift) {
            screen.key(key, &mut chat.history, &cx)
        } else if control && event.key_code == KeyCode::KeyV {
            match keys.clipboard.as_deref_mut().map(|c| c.fetch_text()) {
                Some(bevy::clipboard::ClipboardRead::Ready(Ok(text))) => {
                    screen.type_text(&text, &cx)
                }
                _ => Outcome::Ignored,
            }
        } else if let Some(text) = event.text.as_ref().filter(|_| !control) {
            screen.type_text(text, &cx)
        } else {
            Outcome::Ignored
        };
        match outcome {
            Outcome::Closed(how) => closed = Some(how),
            Outcome::Page(direction) => page += direction,
            Outcome::Copy(text) => {
                if let Some(clipboard) = keys.clipboard.as_deref_mut() {
                    let _ = clipboard.set_text(text);
                }
            }
            Outcome::Ignored | Outcome::Handled => {}
        }
        flush_request(screen, &mut connection);
    }
    let moved = *keys.last_cursor != keys.scene.cursor;
    *keys.last_cursor = keys.scene.cursor;
    if let Some(atlas) = keys.atlas.as_deref() {
        mouse(
            &keys,
            moved,
            screen,
            &mut chat.log,
            shift,
            &atlas.0.font,
            &cx,
        );
        flush_request(screen, &mut connection);
    }
    if page != 0 {
        let lines = per_page(true).saturating_sub(1) as isize;
        chat.log.scroll_by(page * lines, per_page(true));
    }
    keys.keys.reset_all();
    if let Some(closed) = closed {
        close(
            &mut state,
            closed,
            connection.as_deref_mut().map(|c| &mut **c),
        );
        grab(&mut cursor, true);
    }
}

fn flush_request(screen: &mut ChatScreen, connection: &mut Option<Single<&mut ClientConnection>>) {
    if let Some(request) = screen.take_request() {
        send_request(
            connection.as_deref_mut().map(|c| &mut **c),
            screen.input(),
            request,
        );
    }
}

/// Sends what Enter sent, keeps the draft and leaves the screen (CHAT-OPEN-002).
fn close(state: &mut ChatState, closed: Closed, connection: Option<&mut ClientConnection>) {
    let chat = &mut *state.chat;
    let screen = &state.input.0;
    if let Closed::Sent {
        send: Some(sent),
        history_changed,
    } = &closed
    {
        if let Some(connection) = connection {
            send(connection, sent);
        }
        if *history_changed && let Some(file) = &state.history_file {
            write_history(file.0.clone(), chat.history.command_file());
        }
    }
    chat.draft = keeps_draft(screen.input(), Some(&closed), chat.options.save_unsent)
        .then(|| screen.input().to_owned());
    chat.log.reset_scroll();
    *state.screen = Screen::None;
}

fn write_history(path: PathBuf, file: String) {
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            if let Err(error) = std::fs::write(&path, file) {
                warn!("cannot write {}: {error}", path.display());
            }
        })
        .detach();
}

/// The wheel and the left button over the open chat (CHAT-KEY-001, CCMD-LIST-007).
fn mouse(
    keys: &ChatKeys,
    moved: bool,
    screen: &mut ChatScreen,
    log: &mut ChatLog<Text, Vec<crate::gui::font::Span>>,
    shift: bool,
    font: &crate::gui::font::Font,
    cx: &Context<'_, ClientLookup<'_>>,
) {
    let scene = &*keys.scene;
    let rows = list_rows(scene.size, font, screen);
    let over = rows.as_ref().and_then(|rows| {
        rows.rect
            .contains(scene.cursor)
            .then(|| rows.first + ((scene.cursor.y - rows.rect.min.y) / 12) as usize)
    });
    let hovered = rows.and_then(|rows| row_at(rows.rect, scene.cursor).map(|row| rows.first + row));
    let step = keys.scroll.delta.y.clamp(-1.0, 1.0) as isize;
    if step != 0 {
        match (over, screen.commands.list_mut()) {
            (Some(_), Some(list)) => list.scroll(step),
            _ => log.scroll_by(if shift { step } else { step * 7 }, per_page(true)),
        }
    }
    let Some(list) = screen.commands.list_mut() else {
        return;
    };
    if let Some(index) = hovered
        && moved
        && list.selected() != index
    {
        list.select(index as isize);
    }
    if let Some(index) = over.filter(|&index| index < list.entries().len())
        && keys.buttons.just_pressed(MouseButton::Left)
    {
        list.select(index as isize);
        screen.insert_suggestion(cx);
    }
}
