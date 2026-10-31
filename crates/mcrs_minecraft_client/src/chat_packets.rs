//! The observers of the clientbound packets that feed the chat: the log, the overlay, the
//! command tree, the suggestions and the player list.

use bevy::prelude::*;
use mcrs_minecraft_chat_client::Tag;
use mcrs_minecraft_command::Source;
use mcrs_minecraft_game_registries::chat_type::{ChatParameter, ChatType};
use mcrs_minecraft_network::client;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::chat::{CompletionAction, FilterMask};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundCommandSuggestions, ClientboundCommands, ClientboundCustomChatCompletions,
    ClientboundDisguisedChat, ClientboundLogin, ClientboundPlayerChat, ClientboundPlayerInfoRemove,
    ClientboundPlayerInfoUpdate, ClientboundSetActionBarText, ClientboundSystemChatPacket,
};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_resource::ResourceLocation;

use crate::chat::{
    self, Chat, ChatInput, ChatMessage, CommandTree, CommandWorld, MessageOrigin, OnlinePlayer,
    OnlinePlayers,
};

/// The registries whose identifiers the client can name, so their arguments parse on the
/// client.
// chisle: a fixed list; an argument of another registry reads as a type without a grammar and
// asks the server. Registry keys that are not static lift it.
const STATIC_REGISTRIES: [ResourceLocation<'static>; 5] = [
    mcrs_minecraft_biome::keys::BIOME.location(),
    mcrs_minecraft_entity::keys::STRUCTURE.location(),
    mcrs_minecraft_keys::POINT_OF_INTEREST_TYPE,
    mcrs_minecraft_environment::keys::WORLD_CLOCK.location(),
    mcrs_minecraft_environment::keys::TIMELINE.location(),
];

pub(crate) fn on_login(
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

pub(crate) fn on_commands(
    event: On<Received<ClientboundCommands<'static>>>,
    mut tree: ResMut<CommandTree>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    match mcrs_minecraft_command_argument_vanilla::client_tree(
        &packet.nodes,
        packet.root,
        &STATIC_REGISTRIES,
    ) {
        Ok(graph) => tree.0 = graph,
        Err(error) => client::close_connection_later(
            &mut commands,
            event.entity,
            format!("the command tree is invalid: {error}"),
        ),
    }
}

pub(crate) fn on_suggestions(
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
    let cx = chat::context(
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

pub(crate) fn on_system_chat(
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
            origin: MessageOrigin::System,
        });
    }
}

pub(crate) fn on_action_bar(
    event: On<Received<ClientboundSetActionBarText>>,
    mut chat: ResMut<Chat>,
) {
    if let Some(packet) = event.packet() {
        chat.overlay.set(packet.text);
    }
}

/// The `chat` decoration of a chat type around a content.
fn decorate(
    registries: Option<&Registries>,
    chat_type: u32,
    sender: &Text,
    target: Option<&Text>,
    content: Text,
) -> Text {
    let Some(decoration) = registries
        .and_then(|registries| registries.registry::<ChatType>())
        .and_then(|registry| registry.values().get(chat_type as usize))
        .map(|chat_type| &chat_type.chat)
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
    let mut text = Text::translate(decoration.translation_key.clone(), arguments);
    let style = &decoration.style;
    text.style.color = style.color;
    text.style.bold = style.bold;
    text.style.italic = style.italic;
    text.style.underlined = style.underlined;
    text.style.strikethrough = style.strikethrough;
    text.style.obfuscated = style.obfuscated;
    text
}

pub(crate) fn on_player_chat(
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
        client::close_connection_later(
            &mut commands,
            event.entity,
            "multiplayer.disconnect.bad_chat_index".to_owned(),
        );
        return;
    }
    chat.player_chats = chat.player_chats.wrapping_add(1);
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
    if !players.0.iter().any(|player| player.uuid == packet.sender) {
        messages.write(ChatMessage {
            text: decorate(
                Text::translate("chat.validation_error", vec![])
                    .color(Color::RED)
                    .italic(),
            ),
            tag: Some(Tag::Error),
            origin: MessageOrigin::Player,
        });
        return;
    }
    let content = match (&packet.filter, packet.unsigned_content) {
        (FilterMask::FullyFiltered, _) => return,
        (FilterMask::PartiallyFiltered(_), _) => filtered(&packet.content, &packet.filter),
        (_, Some(unsigned)) => unsigned,
        (_, None) => packet.content.0.to_owned().into_text(),
    };
    messages.write(ChatMessage {
        text: decorate(content),
        tag: packet.signature.is_none().then_some(Tag::NotSecure),
        origin: MessageOrigin::Player,
    });
}

/// The body with each hidden code unit as a dark grey `#`.
fn filtered(body: &str, mask: &FilterMask) -> Text {
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
        let units = c.len_utf16();
        let hidden_units = (index..index + units)
            .filter(|&unit| mask.hides(unit))
            .count();
        for unit in index..index + units {
            let hidden = mask.hides(unit);
            if hidden != hidden_run {
                flush(&mut run, hidden_run, &mut text);
                hidden_run = hidden;
            }
            // A pair split by the mask keeps a lone surrogate, which a `str` cannot hold.
            run.push(match (hidden, hidden_units) {
                (true, _) => '#',
                (false, 0) if unit == index => c,
                (false, 0) => continue,
                (false, _) => char::REPLACEMENT_CHARACTER,
            });
        }
        index += units;
    }
    flush(&mut run, hidden_run, &mut text);
    text
}

pub(crate) fn on_disguised_chat(
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
        origin: MessageOrigin::Player,
    });
}

pub(crate) fn on_custom_completions(
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

pub(crate) fn on_player_info_update(
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
        players.0.retain(|player| player.uuid != entry.player_uuid);
        players.0.push(OnlinePlayer {
            uuid: entry.player_uuid,
            name: entry.username.to_string(),
        });
    }
}

pub(crate) fn on_player_info_remove(
    event: On<Received<ClientboundPlayerInfoRemove>>,
    mut players: ResMut<OnlinePlayers>,
) {
    if let Some(packet) = event.packet() {
        players
            .0
            .retain(|player| !packet.players.contains(&player.uuid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::text::TextContent;

    fn shown(body: &str, hidden: u8) -> String {
        let text = filtered(body, &FilterMask::PartiallyFiltered(vec![hidden]));
        text.extra
            .iter()
            .map(|part| match &part.content {
                TextContent::Text { text } => text.as_ref(),
                _ => "",
            })
            .collect()
    }

    #[test]
    fn each_hidden_code_unit_shows_as_one_hash() {
        for (hidden, expected) in [
            (0b0110, "a##b"),
            (0b0010, "a#\u{fffd}b"),
            (0b1001, "#\u{1f600}#"),
        ] {
            assert_eq!(shown("a\u{1f600}b", hidden), expected, "{hidden:#06b}");
        }
    }
}
