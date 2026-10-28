use crate::permission::PermissionCheck;
use crate::set::VanillaArgument;
use crate::source::{Lookup, Source};
use mcrs_command::{Graph, NodeKind, SuggestionList, utf16_position};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::command::{CommandNode, CommandNodeKind, CommandSuggestion};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundCommandSuggestions, ClientboundCommands,
};
use mcrs_minecraft_protocol::text::Text;
use std::borrow::Cow;

pub type VanillaGraph<H> = Graph<VanillaArgument, PermissionCheck, H>;

/// The most suggestions one reply carries (CMDP-SUG-002).
pub(crate) const MAX_SUGGESTIONS: usize = 1000;

fn number(index: usize) -> VarInt {
    VarInt(i32::try_from(index).unwrap_or(i32::MAX))
}

/// The tree the source can see, numbered in the order of the full tree (CMDP-TREE-001,
/// CMDP-TREE-003, CMDP-TREE-004).
pub fn player_tree<H, L: Lookup>(
    graph: &VanillaGraph<H>,
    source: &Source<'_, L>,
) -> ClientboundCommands<'static> {
    let mut numbers: Vec<Option<usize>> = vec![None; graph.node_count()];
    let mut count = 0;
    for (id, node) in graph.nodes() {
        let visible = match node.parent() {
            None => true,
            Some(parent) => {
                numbers.get(parent.index()).copied().flatten().is_some()
                    && node.requirement().passes(source.level)
            }
        };
        if visible && let Some(slot) = numbers.get_mut(id.index()) {
            *slot = Some(count);
            count += 1;
        }
    }
    let number_of = |id: mcrs_command::NodeId| numbers.get(id.index()).copied().flatten();
    let mut nodes = Vec::with_capacity(count);
    for (id, node) in graph.nodes() {
        if number_of(id).is_none() {
            continue;
        }
        let kind = match node.kind() {
            NodeKind::Root => CommandNodeKind::Root,
            NodeKind::Literal(name) => CommandNodeKind::Literal(Cow::Owned(name.to_string())),
            NodeKind::Argument { name, argument } => CommandNodeKind::Argument {
                name: Cow::Owned(name.to_string()),
                parser: argument.wire(),
                suggestions: argument.suggestion_type().map(Into::into),
            },
        };
        nodes.push(CommandNode {
            kind,
            children: node
                .children()
                .iter()
                .filter_map(|&child| number_of(child).map(number))
                .collect(),
            redirect: graph
                .followed_redirect(id, source)
                .and_then(number_of)
                .map(number),
            executable: node.command().is_some(),
            restricted: node.requirement().is_restricted(),
        });
    }
    ClientboundCommands {
        nodes,
        root: VarInt(0),
    }
}

/// The reply to a `command_suggestion` request (CMDP-SUG-002). Positions count UTF-16 code
/// units of `text`, whose suggestions `list` holds.
pub fn suggestion_reply(
    id: VarInt,
    text: &str,
    list: &SuggestionList<'_, '_, &'static str>,
) -> ClientboundCommandSuggestions<'static> {
    let start = utf16_position(text, list.span.start);
    let end = utf16_position(text, list.span.end);
    ClientboundCommandSuggestions {
        id,
        start: number(start),
        length: number(end - start),
        suggestions: list
            .iter()
            .take(MAX_SUGGESTIONS)
            .map(|(suggestion, tooltip)| CommandSuggestion {
                text: Cow::Owned(suggestion.chars().collect()),
                tooltip: tooltip.map(|&key| Text::translate(key, vec![])),
            })
            .collect(),
    }
}
