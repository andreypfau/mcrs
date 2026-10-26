use crate::command::{
    CommandContext, CommandHandlers, CommandTree, Invocation, SourceState, level_of,
};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use mcrs_command::{ParseScratch, Suggestion, build_list};
use mcrs_minecraft_command::{
    ArgumentError, CommandLevel, NoLookup, Source, error_feedback, player_tree, suggestion_reply,
};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text, is_allowed_chat_character};
use mcrs_minecraft_protocol::{GameMode, VarInt};

/// The text any player may ask suggestions for; a player who can edit command blocks may send
/// up to the packet's own limit (CMDP-SUG-001).
const CHAT_SUGGESTION_UNITS: usize = 256;

fn system_message(host: Entity, content: Text) -> OutboundPlayerPacket {
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

/// Raised before a command runs. An observer that cancels it stops the command; the source
/// receives nothing.
#[derive(Event, Debug)]
pub struct CommandEvent {
    pub invocation: Invocation,
    cancelled: bool,
}

impl CommandEvent {
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

pub(crate) fn handle_chat_command(
    event: On<Received<ServerboundChatCommand<'static>>>,
    tree: Res<CommandTree>,
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
        position: transform.translation,
        yaw: transform.rotation.yaw(),
        pitch: transform.rotation.pitch(),
    };
    let lookup = context.lookup();
    let mut invocation = Invocation {
        player: event.entity,
        source: state,
        line: String::new(),
        nodes: Vec::new(),
    };
    let source = invocation.source(&lookup);
    let parse = tree.0.parse(line, 0, &source, &mut scratch);
    let checked = parse.check(&tree.0);
    let context_view = match checked {
        Ok(view) => view,
        Err(error) => {
            for message in error_feedback(line, &error) {
                writer.write(system_message(host, message));
            }
            return;
        }
    };
    let key = context_view
        .last()
        .and_then(|(node, _)| tree.0.node(node))
        .and_then(|node| node.command().copied());
    let Some(key) = key else {
        writer.write(system_message(
            host,
            Text::translate("command.failed", vec![]).color(Color::RED),
        ));
        return;
    };
    invocation.line = line.to_owned();
    invocation.nodes = context_view.nodes().collect();
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
                    Text::translate("command.failed", vec![]).color(Color::RED)
                }
            },
            None => Text::translate("command.failed", vec![]).color(Color::RED),
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
    names: Vec<&'static str>,
}

/// Answers the pending request of each player, one per player per tick, then clears it.
pub(crate) fn answer_suggestion_requests(
    tree: Res<CommandTree>,
    context: CommandContext,
    mut players: Query<
        (
            Entity,
            &PlayerOpLevel,
            &Transform,
            &HostAnchor,
            &mut PendingSuggestion,
        ),
        With<Player>,
    >,
    mut scratch: Local<SuggestionScratch>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let scratch = &mut *scratch;
    for (player, &op_level, transform, &HostAnchor(host), mut pending) in &mut players {
        let Some((id, text)) = pending.0.as_ref() else {
            continue;
        };
        let lookup = context.lookup_with(recycle(std::mem::take(&mut scratch.names)));
        let invocation = Invocation {
            player,
            source: SourceState {
                level: level_of(op_level),
                position: transform.translation,
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            },
            line: String::new(),
            nodes: Vec::new(),
        };
        let source = invocation.source(&lookup);
        let start = usize::from(text.starts_with('/'));
        let parse = tree.0.parse(text, start, &source, &mut scratch.parse);
        let mut suggestions = recycle(std::mem::take(&mut scratch.suggestions));
        tree.0.suggest(&parse, &source, &mut suggestions);
        let list = build_list(text, &mut suggestions);
        writer.write(to(
            host,
            PacketPayload::CommandSuggestions(suggestion_reply(*id, text, &list)),
        ));
        scratch.suggestions = recycle(suggestions);
        scratch.names = recycle(lookup.into_names());
        pending.0 = None;
    }
}

/// Sends a player the tree it can see whenever its level is set, which includes its arrival
/// in the dimension (CMDP-TREE-002).
pub(crate) fn send_command_tree(
    tree: Option<Res<CommandTree>>,
    players: Query<(&PlayerOpLevel, &HostAnchor), Changed<PlayerOpLevel>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(tree) = tree else {
        return;
    };
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
    players: Query<
        'w,
        's,
        (
            Entity,
            &'static HostAnchor,
            &'static PlayerOpLevel,
            &'static GameProfile,
        ),
    >,
}

impl CommandFeedback<'_, '_> {
    /// A packet the command sends besides its messages.
    pub fn send(&mut self, packet: OutboundPlayerPacket) {
        self.writer.write(packet);
    }

    pub fn tell(&mut self, player: Entity, message: Text) {
        if let Ok((_, &HostAnchor(host), _, _)) = self.players.get(player) {
            self.writer.write(system_message(host, message));
        }
    }

    /// A success message; with `broadcast`, every other operator of the dimension sees it too.
    pub fn success(&mut self, invocation: &Invocation, message: Text, broadcast: bool) {
        // chisle: `send_command_feedback` and `log_admin_commands` have no state in the server
        // yet, so both hold their default true; a game rule resource lifts it.
        self.tell(invocation.player, message.clone());
        if !broadcast {
            return;
        }
        let Ok((_, _, _, profile)) = self.players.get(invocation.player) else {
            return;
        };
        let admin = Text::translate(
            "chat.type.admin",
            vec![profile.username.clone().into_text(), message],
        )
        .color(Color::GRAY)
        .italic();
        tracing::info!("[{}: {}]", profile.username, admin.to_legacy_lossy());
        for (entity, &HostAnchor(host), level, _) in &self.players {
            if entity != invocation.player && level_of(*level) > CommandLevel::All {
                self.writer.write(system_message(host, admin.clone()));
            }
        }
    }
}
