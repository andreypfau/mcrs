use bevy_ecs::prelude::{Commands, On, Query, ResMut};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_text::Text;

/// The command lines and the suggestion requests that the players of this world sent in this
/// tick.
#[derive(Resource, Default)]
pub(crate) struct CommandQueue {
    pub(crate) lines: Vec<QueuedLine>,
    pub(crate) suggestions: Vec<QueuedSuggestion>,
    pub(crate) spare: Vec<String>,
}

impl CommandQueue {
    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.suggestions.is_empty()
    }
}

/// A command line in the order of arrival, from the player of the session `player`.
pub(crate) struct QueuedLine {
    pub(crate) player: PlayerSession,
    pub(crate) line: String,
}

/// The latest suggestion request of the player of the session `player`.
pub(crate) struct QueuedSuggestion {
    pub(crate) player: PlayerSession,
    pub(crate) id: VarInt,
    pub(crate) text: String,
}

fn allowed(line: &str) -> bool {
    line.chars()
        .all(mcrs_minecraft_text::is_allowed_chat_character)
}

/// Disconnects a player whose command line holds a code unit no player can type. The muxer
/// owns the connection; the world of the player drops the line.
pub(crate) fn reject_illegal_command_line(
    event: On<Received<ServerboundChatCommand<'static>>>,
    mut connections: Query<&mut ServerSideConnection>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if allowed(packet.command.0) {
        return;
    }
    let Ok(mut connection) = connections.get_mut(event.entity) else {
        return;
    };
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

/// Queues a command line of a player of this world for the commands of this tick.
pub(crate) fn queue_command_line(
    event: On<Received<ServerboundChatCommand<'static>>>,
    players: Query<&Owner>,
    mut queue: ResMut<CommandQueue>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let line = packet.command.0;
    let Ok(&Owner(player)) = players.get(event.entity) else {
        return;
    };
    if !allowed(line) {
        return;
    }
    let mut owned = queue.spare.pop().unwrap_or_default();
    owned.clear();
    owned.push_str(line);
    queue.lines.push(QueuedLine {
        player,
        line: owned,
    });
}

/// Keeps the latest suggestion request of each player of this world for the commands of this
/// tick. A newer request replaces an older one, and the replaced one gets no reply.
pub(crate) fn queue_suggestion_request(
    event: On<Received<ServerboundCommandSuggestion<'static>>>,
    players: Query<&Owner>,
    mut queue: ResMut<CommandQueue>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok(&Owner(player)) = players.get(event.entity) else {
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
