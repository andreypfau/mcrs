use crate::command_outbox::CommandOutbox;
use crate::command_queue::QueuedSuggestion;
use crate::command_set::CommandSet;
use crate::context::CommandContext;
use crate::source_state::{PlayerLevels, player_source};
use bevy_ecs::prelude::{Local, ResMut};
use bevy_ecs::system::InRef;
use mcrs_command::{ParseScratch, Suggestion, SuggestionList};
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_engine::bus::{self, PacketPayload};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::CommandSuggestion;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundCommandSuggestions;
use mcrs_minecraft_wire::utf16_len;
use std::borrow::Cow;

/// The text any player may ask suggestions for; a player who can edit command blocks may send
/// up to the packet's own limit.
const CHAT_SUGGESTION_UNITS: usize = 256;

pub(crate) struct SuggestionScratch<E> {
    parse: ParseScratch<E>,
    suggestions: Vec<Suggestion<'static, &'static str>>,
}

impl<E> Default for SuggestionScratch<E> {
    fn default() -> Self {
        Self {
            parse: ParseScratch::default(),
            suggestions: Vec::new(),
        }
    }
}

/// Answers a queued suggestion request in the world of its source dimension, with the graph and
/// the targets lent to it. The reply goes to the outbox.
pub(crate) fn answer_suggestion<S: CommandSet>(
    queued: InRef<QueuedSuggestion>,
    context: CommandContext<S>,
    levels: PlayerLevels,
    mut outbox: ResMut<CommandOutbox>,
    mut scratch: Local<SuggestionScratch<S::Error>>,
) {
    let QueuedSuggestion { player, id, text } = &*queued;
    let Some((state, mode)) = player_source(context.targets(), &levels, *player) else {
        return;
    };
    let edits_command_blocks =
        mode == GameMode::Creative && state.level >= CommandLevel::Gamemasters;
    if !edits_command_blocks && utf16_len(text) > CHAT_SUGGESTION_UNITS {
        return;
    }
    let (Some(graph), Ok(source)) = (context.tree.graph(), context.source_of(&state)) else {
        return;
    };
    let scratch = &mut *scratch;
    let start = usize::from(text.starts_with('/'));
    let parse = graph.parse(text, start, &source, &mut scratch.parse);
    let mut suggestions = mcrs_command::recycle(std::mem::take(&mut scratch.suggestions));
    graph.suggest(&parse, &source, &mut suggestions);
    let list = SuggestionList::build(text, &mut suggestions);
    let reply = mcrs_minecraft_command::suggestion_reply(text, &list);
    let packet = ClientboundCommandSuggestions {
        id: *id,
        start: reply.start,
        length: reply.length,
        suggestions: reply
            .entries
            .into_iter()
            .map(|entry| CommandSuggestion {
                text: Cow::Owned(entry.text),
                tooltip: entry
                    .tooltip
                    .and_then(|tooltip| mcrs_minecraft_protocol::text::named(&tooltip).ok()),
            })
            .collect(),
    };
    outbox
        .packets
        .push(bus::to(*player, PacketPayload::CommandSuggestions(packet)));
    scratch.suggestions = mcrs_command::recycle(suggestions);
}
