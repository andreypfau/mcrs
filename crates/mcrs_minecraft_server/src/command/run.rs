use crate::command::effect::{CommandOutbox, Effect, Teleport};
use crate::command::seam::{PendingLine, PendingLines};
use crate::command::{
    CommandContext, CommandTargets, CommandTree, Invocation, SourceState, level_of, tree_packet,
};
use crate::ops::OpList;
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use mcrs_command::{Chain, ParseScratch, Suggestion, build_list};
use mcrs_minecraft_command::{ArgumentError, CommandLevel, error_feedback, suggestion_reply};
use mcrs_minecraft_engine::bus::{
    OperatorBroadcast, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, to,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{PlayerSession, Session};
use mcrs_minecraft_level::world::sub_app::DimensionLabel;
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

/// Parses a command line and leaves it for the host, which runs it with every dimension in
/// reach.
pub(crate) fn handle_chat_command(
    event: On<Received<ServerboundChatCommand<'static>>>,
    context: CommandContext,
    players: Query<(&PlayerOpLevel, &Transform, &HostAnchor), With<Player>>,
    label: Option<Res<DimensionLabel>>,
    mut pending: ResMut<PendingLines>,
    mut scratch: Local<ParseScratch<ArgumentError>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let line = packet.command.0;
    let (Ok((&op_level, transform, &HostAnchor(host))), Some(label)) =
        (players.get(event.entity), label)
    else {
        return;
    };
    if !line.chars().all(is_allowed_chat_character) {
        return;
    }
    let state = SourceState::of_player(host, label.0, level_of(op_level), transform);
    let source = context.source_of(&state);
    let tree = &context.tree;
    let parse = tree.0.parse(line, 0, &source, &mut scratch);
    if let Err(error) = parse.check(&tree.0) {
        for message in error_feedback(line, &error) {
            writer.write(system_message(host, message));
        }
        return;
    }
    let mut chain = Chain::default();
    chain.copy_from(&tree.0, &parse);
    pending.0.push(PendingLine {
        output: host,
        event: CommandEvent {
            player: event.entity,
            source: state,
            line: line.to_owned(),
            cancelled: false,
        },
        chain,
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
    label: Option<Res<DimensionLabel>>,
    mut scratch: Local<SuggestionScratch>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let scratch = &mut *scratch;
    let dimension = label.map_or(Entity::PLACEHOLDER, |label| label.0);
    for (&op_level, transform, &HostAnchor(host), mut pending) in &mut players {
        let Some((id, text)) = pending.0.as_ref() else {
            continue;
        };
        let state = SourceState::of_player(host, dimension, level_of(op_level), transform);
        let source = context.source_of(&state);
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

/// Sends a player the tree it can see whenever its level changes. A player that spawns or
/// changes dimension receives it in the sequence that brings it in (CMDP-TREE-002).
pub(crate) fn send_command_tree(
    tree: Res<CommandTree>,
    players: Query<(Ref<PlayerOpLevel>, &HostAnchor), Changed<PlayerOpLevel>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (op_level, &HostAnchor(host)) in &players {
        if !op_level.is_added() {
            writer.write(tree_packet(&tree.0, host, *op_level));
        }
    }
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
        self.send(system_message(invocation.output, message.clone()));
        if !broadcast {
            return;
        }
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

    /// Moves the player with the host anchor `player`, in any dimension.
    pub fn teleport(&mut self, player: Entity, to: Teleport) {
        self.outbox.effects.push(Effect::Teleport { player, to });
    }

    /// Sets the game mode of the player with the host anchor `player`, in any dimension.
    pub fn set_game_mode(&mut self, player: Entity, mode: GameMode) {
        self.outbox.effects.push(Effect::GameMode { player, mode });
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
