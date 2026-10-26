use crate::command::{
    CommandContext, CommandFailure, CommandHandlers, CommandTree, Invocation, SourceState, level_of,
};
use crate::ops::OpList;
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use mcrs_command::{ParseScratch, Suggestion, build_list};
use mcrs_minecraft_command::{
    ArgumentError, CommandLevel, NoLookup, Source, error_feedback, player_tree, suggestion_reply,
};
use mcrs_minecraft_engine::bus::{
    OperatorBroadcast, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, to,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{PlayerSession, Session};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text, is_allowed_chat_character};
use mcrs_minecraft_protocol::{GameMode, VarInt};
use smallvec::SmallVec;

/// The text any player may ask suggestions for; a player who can edit command blocks may send
/// up to the packet's own limit (CMDP-SUG-001).
const CHAT_SUGGESTION_UNITS: usize = 256;

pub(crate) fn system_message(host: Entity, content: Text) -> OutboundPlayerPacket {
    to(
        host,
        PacketPayload::SystemChat(ClientboundSystemChatPacket {
            content,
            overlay: false,
        }),
    )
}

/// The host disconnects a player whose command line holds a code unit no player can type
/// (CMDP-EXEC-002); the dimension drops the line.
pub(crate) fn reject_illegal_characters(
    event: On<Received<ServerboundChatCommand<'static>>>,
    mut connections: Query<&mut ServerSideConnection>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if packet.command.0.chars().all(is_allowed_chat_character) {
        return;
    }
    if let Ok(mut connection) = connections.get_mut(event.entity) {
        connection
            .disconnect(
                ConnectionState::Game,
                Text::translate("multiplayer.disconnect.illegal_characters", vec![]),
            )
            .ok();
        commands
            .entity(event.entity)
            .remove::<ServerSideConnection>();
    }
}

/// Raised before a command runs. When an observer sets `cancelled`, the command does not run
/// and the source receives nothing.
#[derive(Event, Debug)]
pub struct CommandEvent {
    invocation: Invocation,
    pub cancelled: bool,
}

impl CommandEvent {
    /// Read-only: the parse that chose the branch belongs to this line.
    pub fn invocation(&self) -> &Invocation {
        &self.invocation
    }
}

pub(crate) fn handle_chat_command(
    event: On<Received<ServerboundChatCommand<'static>>>,
    context: CommandContext,
    players: Query<(&PlayerOpLevel, &Transform, &HostAnchor), With<Player>>,
    mut scratch: Local<ParseScratch<ArgumentError>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let line = packet.command.0;
    let Ok((&op_level, transform, &HostAnchor(host))) = players.get(event.entity) else {
        return;
    };
    if !line.chars().all(is_allowed_chat_character) {
        return;
    }
    let state = SourceState {
        level: level_of(op_level),
        transform: *transform,
    };
    let source = state.source(&context);
    let tree = &context.tree;
    let parse = tree.0.parse(line, 0, &source, &mut scratch);
    let (context_view, &handler) = match parse.check(&tree.0) {
        Ok(checked) => checked,
        Err(error) => {
            for message in error_feedback(line, &error) {
                writer.write(system_message(host, message));
            }
            return;
        }
    };
    let invocation = Invocation {
        player: event.entity,
        source: state,
        line: line.to_owned(),
        nodes: context_view.nodes().collect(),
        branch: handler.branch,
    };
    let key = handler.key;
    commands.queue(move |world: &mut World| {
        let mut event = CommandEvent {
            invocation,
            cancelled: false,
        };
        world.trigger_ref(&mut event);
        if event.cancelled {
            return;
        }
        let failure = match world
            .get_resource::<CommandHandlers>()
            .and_then(|handlers| handlers.get(key))
        {
            Some(handler) => match world.run_system_with(handler, event.invocation) {
                Ok(Ok(_)) => return,
                Ok(Err(failure)) => failure.text(),
                Err(error) => {
                    tracing::error!("a command handler did not run: {error}");
                    CommandFailure::failed().text()
                }
            },
            None => CommandFailure::failed().text(),
        };
        world.write_message(system_message(host, failure));
    });
}

/// The latest suggestion request of a player that has no reply yet (CMDP-SUG-003). A newer
/// request replaces it, and the replaced one gets no reply.
#[derive(Component, Debug, Default)]
pub(crate) struct PendingSuggestion(Option<(VarInt, String)>);

pub(crate) fn queue_suggestion_request(
    event: On<Received<ServerboundCommandSuggestion<'static>>>,
    mut players: Query<(&PlayerOpLevel, &PlayerGameMode, &mut PendingSuggestion), With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((&op_level, mode, mut pending)) = players.get_mut(event.entity) else {
        return;
    };
    let text = packet.text.0;
    let edits_command_blocks =
        mode.0 == GameMode::Creative && level_of(op_level) >= CommandLevel::Gamemasters;
    if !edits_command_blocks && text.encode_utf16().count() > CHAT_SUGGESTION_UNITS {
        return;
    }
    match &mut pending.0 {
        Some((id, stored)) => {
            *id = packet.id;
            stored.clear();
            stored.push_str(text);
        }
        slot @ None => *slot = Some((packet.id, text.to_owned())),
    }
}

pub(crate) fn has_pending_suggestion(
    pending: Query<&PendingSuggestion, Changed<PendingSuggestion>>,
) -> bool {
    pending.iter().any(|pending| pending.0.is_some())
}

/// Moves the items out of `buffer` into a buffer of another lifetime, keeping its allocation:
/// the two element types differ only in lifetimes, so the collect reuses the buffer in place.
fn recycle<A, B>(buffer: Vec<A>) -> Vec<B> {
    buffer.into_iter().filter_map(|_| None).collect()
}

#[derive(Default)]
pub(crate) struct SuggestionScratch {
    parse: ParseScratch<ArgumentError>,
    suggestions: Vec<Suggestion<'static, &'static str>>,
}

/// Answers the pending request of each player, one per player per tick, then clears it.
pub(crate) fn answer_suggestion_requests(
    context: CommandContext,
    mut players: Query<
        (
            &PlayerOpLevel,
            &Transform,
            &HostAnchor,
            &mut PendingSuggestion,
        ),
        (With<Player>, Changed<PendingSuggestion>),
    >,
    mut scratch: Local<SuggestionScratch>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let scratch = &mut *scratch;
    for (&op_level, transform, &HostAnchor(host), mut pending) in &mut players {
        let Some((id, text)) = pending.0.as_ref() else {
            continue;
        };
        let state = SourceState {
            level: level_of(op_level),
            transform: *transform,
        };
        let source = state.source(&context);
        let start = usize::from(text.starts_with('/'));
        let parse = context
            .tree
            .0
            .parse(text, start, &source, &mut scratch.parse);
        let mut suggestions = recycle(std::mem::take(&mut scratch.suggestions));
        context.tree.0.suggest(&parse, &source, &mut suggestions);
        let list = build_list(text, &mut suggestions);
        writer.write(to(
            host,
            PacketPayload::CommandSuggestions(suggestion_reply(*id, text, &list)),
        ));
        scratch.suggestions = recycle(suggestions);
        pending.0 = None;
    }
}

/// Sends a player the tree it can see whenever its level is set, which includes its arrival
/// in the dimension (CMDP-TREE-002).
pub(crate) fn send_command_tree(
    tree: Res<CommandTree>,
    players: Query<(&PlayerOpLevel, &HostAnchor), Changed<PlayerOpLevel>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (&op_level, &HostAnchor(host)) in &players {
        let source = Source::new(level_of(op_level), &NoLookup);
        writer.write(to(host, PacketPayload::Commands(player_tree(&tree.0, &source))).critical());
    }
}

/// Sends what a command says to its source and to the operators who watch it
/// (CMDP-EXEC-004).
#[derive(SystemParam)]
pub struct CommandFeedback<'w, 's> {
    writer: MessageWriter<'w, OutboundPlayerPacket>,
    broadcasts: MessageWriter<'w, OperatorBroadcast>,
    players: Query<'w, 's, (&'static HostAnchor, &'static GameProfile)>,
}

impl CommandFeedback<'_, '_> {
    /// A packet the command sends besides its messages.
    pub fn send(&mut self, packet: OutboundPlayerPacket) {
        self.writer.write(packet);
    }

    pub fn tell(&mut self, player: Entity, message: Text) {
        if let Ok((&HostAnchor(host), _)) = self.players.get(player) {
            self.writer.write(system_message(host, message));
        }
    }

    /// A success message; with `broadcast`, every other operator sees it too.
    pub fn success(&mut self, invocation: &Invocation, message: Text, broadcast: bool) {
        // chisle: `send_command_feedback` and `log_admin_commands` have no state in the server
        // yet, so both hold their default true; a game rule resource lifts it.
        self.tell(invocation.player, message.clone());
        if !broadcast {
            return;
        }
        let Ok((&HostAnchor(source), profile)) = self.players.get(invocation.player) else {
            return;
        };
        // The server has no language table, so the log keeps the translation key and its
        // arguments as JSON.
        tracing::info!("[{}: {message}]", profile.username);
        let admin = Text::translate(
            "chat.type.admin",
            vec![profile.username.clone().into_text(), message],
        )
        .color(Color::GRAY)
        .italic();
        self.broadcasts.write(OperatorBroadcast {
            source,
            message: admin,
        });
    }
}

/// Delivers an admin message from any dimension to every online operator but its source. The
/// host holds each session's profile and the operator list, so the set is read here when a
/// message arrives.
pub(crate) fn broadcast_to_operators(
    broadcast: On<OperatorBroadcast>,
    sessions: Query<(Entity, &GameProfile), With<Session>>,
    ops: Option<Res<OpList>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(ops) = ops else {
        return;
    };
    let operators: SmallVec<[Entity; 8]> = sessions
        .iter()
        .filter(|&(anchor, profile)| anchor != broadcast.source && ops.get(&profile.id).is_some())
        .map(|(anchor, _)| anchor)
        .collect();
    if operators.is_empty() {
        return;
    }
    writer.write(OutboundPlayerPacket {
        target: PacketTarget::PlayerSet(operators),
        priority: PacketPriority::Normal,
        data: PacketPayload::SystemChat(ClientboundSystemChatPacket {
            content: broadcast.message.clone(),
            overlay: false,
        }),
        session: PlayerSession(0),
        epoch: 0,
    });
}
