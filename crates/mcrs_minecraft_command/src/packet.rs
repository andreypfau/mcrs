use crate::permission::PermissionCheck;
use crate::source::Source;
use mcrs_command::{Graph, NodeId, NodeKind, SuggestionList};
use mcrs_minecraft_command_tree::{ArgumentParser, CommandNode, CommandNodeKind};
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::{VarInt, utf16_position};
use std::borrow::Cow;

pub trait WireArgument {
    type WireError: core::error::Error + Send + Sync + 'static;

    fn wire(&self) -> Result<ArgumentParser<'static>, Self::WireError>;

    fn suggestion_type(&self) -> Option<ResourceLocation<'static>>;
}

const MAX_SUGGESTIONS: usize = 1000;

fn number(index: usize) -> VarInt {
    VarInt(i32::try_from(index).unwrap_or(i32::MAX))
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerTree<'a> {
    pub nodes: Vec<CommandNode<'a>>,
    pub root: VarInt,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionEntry {
    pub text: String,
    pub tooltip: Option<Text>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionReply {
    pub start: VarInt,
    pub length: VarInt,
    pub entries: Vec<SuggestionEntry>,
}

pub fn player_tree<'a, A: WireArgument, H, L>(
    graph: &'a Graph<A, PermissionCheck, H>,
    source: &Source<'_, L>,
) -> Result<PlayerTree<'a>, A::WireError> {
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
    let number_of = |id: NodeId| numbers.get(id.index()).copied().flatten();
    let mut nodes = Vec::with_capacity(count);
    for (id, node) in graph.nodes() {
        if number_of(id).is_none() {
            continue;
        }
        let kind = match node.kind() {
            NodeKind::Root => CommandNodeKind::Root,
            NodeKind::Literal(name) => CommandNodeKind::Literal(Cow::Borrowed(name)),
            NodeKind::Argument { name, argument } => CommandNodeKind::Argument {
                name: Cow::Borrowed(name),
                parser: argument.wire()?,
                suggestions: argument.suggestion_type(),
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
    Ok(PlayerTree {
        nodes,
        root: VarInt(0),
    })
}

pub fn suggestion_reply(
    text: &str,
    list: &SuggestionList<'_, '_, &'static str>,
) -> SuggestionReply {
    let start = utf16_position(text, list.span.start);
    let end = utf16_position(text, list.span.end);
    SuggestionReply {
        start: number(start),
        length: number(end - start),
        entries: list
            .iter()
            .take(MAX_SUGGESTIONS)
            .map(|(suggestion, tooltip)| SuggestionEntry {
                text: suggestion.chars().collect(),
                tooltip: tooltip.map(|&key| Text::translate(key, vec![])),
            })
            .collect(),
    }
}
