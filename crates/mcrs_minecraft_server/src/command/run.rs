use crate::command::effect::OperatorBroadcast;
use crate::command::effect::{CommandOutbox, EffectQueue, apply_next};
use crate::command::seam::{CommandQueue, QueuedLine, QueuedSuggestion};
use crate::command::{CommandContext, CommandTargets, Handler, Invocation, SourceState, level_of};
use crate::ops::OpList;
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{InRef, SystemParam};
use mcrs_command::{Chain, ParseScratch, Suggestion, build_list, recycle};
use mcrs_minecraft_command::{ArgumentError, CommandLevel, error_feedback, suggestion_reply};
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketTarget, to, to_target,
};
use mcrs_minecraft_engine::session::{GameProfile, HostAnchorRef};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::Session;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text, is_allowed_chat_character};
use smallvec::SmallVec;

/// The text any player may ask suggestions for; a player who can edit command blocks may send
/// up to the packet's own limit (CMDP-SUG-001).
const CHAT_SUGGESTION_UNITS: usize = 256;

pub(crate) fn system_message(host: Entity, content: Text) -> OutboundPlayerPacket {
    to(host, system_chat(content))
}

pub(crate) fn system_chat(content: Text) -> PacketPayload {
    PacketPayload::SystemChat(ClientboundSystemChatPacket {
        content,
        overlay: false,
    })
}

/// Queues a command line for the seam. The host disconnects a player whose command line holds a
/// code unit no player can type (CMDP-EXEC-002), and drops the line.
pub(crate) fn queue_command_line(
    event: On<Received<ServerboundChatCommand<'static>>>,
    mut connections: Query<(Option<&HostAnchorRef>, Option<&mut ServerSideConnection>)>,
    mut queue: ResMut<CommandQueue>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((anchor, connection)) = connections.get_mut(event.entity) else {
        return;
    };
    let line = packet.command.0;
    if line.chars().all(is_allowed_chat_character) {
        if let Some(&HostAnchorRef(player)) = anchor {
            queue.lines.push(QueuedLine {
                player,
                line: line.to_owned(),
            });
        }
        return;
    }
    if let Some(mut connection) = connection {
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

/// Keeps the latest suggestion request of each player for the seam (CMDP-SUG-003). A newer
/// request replaces an older one, and the replaced one gets no reply.
pub(crate) fn queue_suggestion_request(
    event: On<Received<ServerboundCommandSuggestion<'static>>>,
    connections: Query<&HostAnchorRef>,
    mut queue: ResMut<CommandQueue>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok(&HostAnchorRef(player)) = connections.get(event.entity) else {
        return;
    };
    let text = packet.text.0;
    match queue
        .suggestions
        .iter_mut()
        .find(|request| request.player == player)
    {
        Some(request) => {
            request.id = packet.id;
            request.text.clear();
            request.text.push_str(text);
        }
        None => queue.suggestions.push(QueuedSuggestion {
            player,
            id: packet.id,
            text: text.to_owned(),
        }),
    }
}

/// Raised in the world of the source dimension before a command line runs. When an observer
/// sets `cancelled`, the command does not run and the source receives nothing.
#[derive(Event, Debug)]
pub struct CommandEvent {
    pub(crate) player: Entity,
    pub(crate) source: SourceState,
    pub(crate) line: String,
    pub cancelled: bool,
}

/// Read-only: the parse of the chain belongs to this line and this source.
impl CommandEvent {
    pub fn player(&self) -> Entity {
        self.player
    }

    pub fn source(&self) -> &SourceState {
        &self.source
    }

    pub fn line(&self) -> &str {
        &self.line
    }
}

/// The source of the player with the host anchor `player`, from its entry in the targets and its
/// level in this world.
fn player_source(
    targets: &CommandTargets,
    levels: &Query<(&PlayerOpLevel, &PlayerGameMode), With<Player>>,
    player: Entity,
) -> Option<(SourceState, GameMode)> {
    let entry = targets.player(player)?;
    let dimension = targets.label_of(entry.dimension)?;
    let (&op_level, mode) = levels.get(entry.entity).ok()?;
    let state = SourceState::of_player(player, dimension, level_of(op_level), &entry.transform);
    Some((state, mode.0))
}

/// A queued line and the chain its parse fills, which the seam reuses from line to line.
pub(crate) struct LineToParse {
    pub(crate) queued: QueuedLine,
    pub(crate) chain: Chain<Handler>,
}

/// Parses a queued line in the world of its source dimension, with the graph and the targets
/// lent to it. A parse error goes to the outbox, and the line does not run.
pub(crate) fn parse_line(
    input: InMut<LineToParse>,
    context: CommandContext,
    levels: Query<(&PlayerOpLevel, &PlayerGameMode), With<Player>>,
    mut outbox: ResMut<CommandOutbox>,
    mut scratch: Local<ParseScratch<ArgumentError>>,
) -> Option<SourceState> {
    let LineToParse {
        queued: QueuedLine { player, line },
        chain,
    } = input.0;
    let (state, _) = player_source(context.targets(), &levels, *player)?;
    let graph = context.tree.graph()?;
    let source = context.source_of(&state).ok()?;
    let parse = graph.parse(line, 0, &source, &mut scratch);
    if let Err(error) = parse.check(graph) {
        for message in error_feedback(line, &error) {
            outbox.packets.push(system_message(*player, message));
        }
        return None;
    }
    chain.copy_from(graph, &parse);
    Some(state)
}

#[derive(Default)]
pub(crate) struct SuggestionScratch {
    parse: ParseScratch<ArgumentError>,
    suggestions: Vec<Suggestion<'static, &'static str>>,
}

/// Answers a queued suggestion request in the world of its source dimension, with the graph and
/// the targets lent to it. The reply goes to the outbox.
pub(crate) fn answer_suggestion(
    queued: InRef<QueuedSuggestion>,
    context: CommandContext,
    levels: Query<(&PlayerOpLevel, &PlayerGameMode), With<Player>>,
    mut outbox: ResMut<CommandOutbox>,
    mut scratch: Local<SuggestionScratch>,
) {
    let QueuedSuggestion { player, id, text } = &*queued;
    let Some((state, mode)) = player_source(context.targets(), &levels, *player) else {
        return;
    };
    let edits_command_blocks =
        mode == GameMode::Creative && state.level >= CommandLevel::Gamemasters;
    if !edits_command_blocks && text.encode_utf16().count() > CHAT_SUGGESTION_UNITS {
        return;
    }
    let (Some(graph), Ok(source)) = (context.tree.graph(), context.source_of(&state)) else {
        return;
    };
    let scratch = &mut *scratch;
    let start = usize::from(text.starts_with('/'));
    let parse = graph.parse(text, start, &source, &mut scratch.parse);
    let mut suggestions = recycle(std::mem::take(&mut scratch.suggestions));
    graph.suggest(&parse, &source, &mut suggestions);
    let list = build_list(text, &mut suggestions);
    outbox.packets.push(to(
        *player,
        PacketPayload::CommandSuggestions(suggestion_reply(*id, text, &list)),
    ));
    scratch.suggestions = recycle(suggestions);
}

/// What a handler sends and changes. Its messages go out after its effects apply, so a player
/// that a command moves hears of the move after the move (CMD-TP-014).
#[derive(SystemParam)]
pub struct CommandOutput<'w> {
    outbox: ResMut<'w, CommandOutbox>,
    targets: Res<'w, CommandTargets>,
}

impl CommandOutput<'_> {
    /// A packet the command sends besides its messages.
    pub fn send(&mut self, packet: OutboundPlayerPacket) {
        self.outbox.packets.push(packet);
    }

    /// A system message to the player with the host anchor `player`, in any dimension.
    pub fn tell(&mut self, player: Entity, message: Text) {
        self.send(system_message(player, message));
    }

    /// A success message; with `broadcast`, every other operator sees it too.
    pub fn success(&mut self, invocation: &Invocation, message: Text, broadcast: bool) {
        // chisle: `send_command_feedback` and `log_admin_commands` have no state in the server
        // yet, so both hold their default true; a game rule resource lifts it.
        if !broadcast {
            self.send(system_message(invocation.output, message));
            return;
        }
        self.send(system_message(invocation.output, message.clone()));
        let Some(source) = invocation
            .source
            .entity
            .and_then(|anchor| self.targets.player(anchor))
        else {
            return;
        };
        // The server has no language table, so the log keeps the translation key and its
        // arguments as JSON.
        tracing::info!("[{}: {message}]", source.name);
        let admin = Text::translate(
            "chat.type.admin",
            vec![source.name.clone().into_text(), message],
        )
        .color(Color::GRAY)
        .italic();
        self.outbox.broadcasts.push(OperatorBroadcast {
            source: invocation.output,
            message: admin,
        });
    }

    /// A source a modifier gives, after the ones it gave before in this run. The modifier then
    /// returns [`Modified::Forked`](crate::command::Modified::Forked).
    pub fn fork(&mut self, source: SourceState) {
        self.outbox.forked.push(source);
    }

    /// An effect for the host to apply after this run, in the order of writing across all
    /// effect types, before the messages of the run go out.
    pub fn effect<E: Send + Sync + 'static>(&mut self, queue: &mut EffectQueue<E>, effect: E) {
        queue.values.push_back(effect);
        self.outbox.effects.push(apply_next::<E>);
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
    writer.write(to_target(
        PacketTarget::PlayerSet(operators),
        system_chat(broadcast.message.clone()),
    ));
}
