use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, On, Query, ResMut};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine::session::HostAnchorRef;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::Text;

/// The command lines and the suggestion requests that the host received since the last pass.
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

/// A command line in the order of arrival, from the player with the host anchor `player`.
pub(crate) struct QueuedLine {
    pub(crate) player: Entity,
    pub(crate) line: String,
}

/// The latest suggestion request of the player with the host anchor `player`.
pub(crate) struct QueuedSuggestion {
    pub(crate) player: Entity,
    pub(crate) id: VarInt,
    pub(crate) text: String,
}

/// Queues a command line for the next pass. The host disconnects a player whose command line holds a
/// code unit no player can type, and drops the line.
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
    if line.chars().all(mcrs_command::is_allowed_chat_character) {
        if let Some(&HostAnchorRef(player)) = anchor {
            let mut owned = queue.spare.pop().unwrap_or_default();
            owned.clear();
            owned.push_str(line);
            queue.lines.push(QueuedLine {
                player,
                line: owned,
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

/// Keeps the latest suggestion request of each player for the next pass. A newer
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
