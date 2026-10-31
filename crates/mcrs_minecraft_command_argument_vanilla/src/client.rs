use crate::error::{ArgumentError, CommandSyntaxError};
use crate::vanilla_argument::VanillaArgument;
use crate::vanilla_lookup::VanillaLookup;
use core::fmt;
use mcrs_command::{
    ArgumentSet, BuildError, ContextView, Graph, GraphBuilder, NodeId, Reader, SuggestionBuilder,
};
use mcrs_minecraft_command::{Lookup, Source, identifier_matches};
use mcrs_minecraft_command_tree::{ArgumentParser, CommandNode, CommandNodeKind};
use mcrs_minecraft_keys::command_argument_type as parser;
use mcrs_minecraft_resource::StaticResourceLocation;
use mcrs_minecraft_wire::VarInt;
use std::collections::VecDeque;

const SOUND_EVENT: StaticResourceLocation = mcrs_minecraft_resource::rl!("minecraft:sound_event");

/// The client tree of the last `commands` packet. The client source holds the
/// restricted atom, so every node passes; an executable node runs nothing on the client.
pub type ClientGraph = Graph<ClientArgument, (), ()>;

/// Who makes the suggestions of an argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionType {
    /// The client suggests from the grammar of the argument.
    OfType,
    /// The client asks the server.
    AskServer,
    /// The client suggests the sound events it knows.
    AvailableSounds,
}

impl From<&str> for SuggestionType {
    fn from(name: &str) -> Self {
        match name {
            "minecraft:available_sounds" => SuggestionType::AvailableSounds,
            // chisle: summonable_entities and post_effects ask the server, because the client
            // holds no summonable entity set and no post effect list yet.
            _ => SuggestionType::AskServer,
        }
    }
}

/// An argument of the client tree: the grammar of its parser id, or `None` when the client
/// lacks that grammar and reads the rest of the line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientArgument {
    pub grammar: Option<VanillaArgument>,
    pub suggestions: SuggestionType,
    /// The parser id is `minecraft:message`.
    pub message: bool,
}

impl ClientArgument {
    /// The argument takes its suggestions from a server request.
    pub fn needs_server<L: Lookup>(&self, lookup: &L) -> bool {
        let Some(grammar) = self.grammar else {
            return true;
        };
        match self.suggestions {
            SuggestionType::AskServer => return true,
            SuggestionType::AvailableSounds => return !lookup.has_registry(SOUND_EVENT),
            SuggestionType::OfType => {}
        }
        grammar
            .registry()
            .is_some_and(|registry| !lookup.has_registry(registry))
    }
}

impl<L: VanillaLookup> ArgumentSet<Source<'_, L>> for ClientArgument {
    type Error = ArgumentError;
    type Tooltip = &'static str;

    fn check(
        &self,
        reader: &mut Reader<'_>,
        source: &Source<'_, L>,
    ) -> Result<(), CommandSyntaxError> {
        match &self.grammar {
            Some(grammar) => grammar.check(reader, source),
            None => {
                reader.set_cursor(reader.input().len());
                Ok(())
            }
        }
    }

    fn suggest<'t>(
        &'t self,
        source: &'t Source<'_, L>,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, &'static str>,
    ) {
        match (&self.grammar, self.suggestions) {
            (Some(grammar), SuggestionType::OfType) => grammar.suggest(source, context, builder),
            (Some(_), SuggestionType::AvailableSounds) => {
                builder.suggest_matching(source.lookup.entries(SOUND_EVENT), identifier_matches);
            }
            _ => {}
        }
    }
}

/// Why the client rejects a `commands` packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TreeError {
    /// A child or redirect index, or the root index, is out of range.
    Index(usize),
    /// The root index does not name a root node.
    NotRoot,
    /// The child links of this node lead back to it.
    ChildCycle(usize),
    /// A redirect target that no child link reaches.
    Unreachable(usize),
    /// The graph builder rejects the tree.
    Build(BuildError),
}

impl From<BuildError> for TreeError {
    fn from(error: BuildError) -> Self {
        TreeError::Build(error)
    }
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::Index(index) => write!(f, "node index {index} is out of range"),
            TreeError::NotRoot => f.write_str("the root index does not name a root node"),
            TreeError::ChildCycle(index) => write!(f, "the children of node {index} form a cycle"),
            TreeError::Unreachable(index) => {
                write!(
                    f,
                    "node {index} is a redirect target that no child link reaches"
                )
            }
            TreeError::Build(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for TreeError {}

fn index(value: VarInt, len: usize) -> Result<usize, TreeError> {
    usize::try_from(value.0)
        .ok()
        .filter(|&index| index < len)
        .ok_or(TreeError::Index(value.0.unsigned_abs() as usize))
}

fn argument(
    parser: &ArgumentParser<'_>,
    suggestions: SuggestionType,
    registries: &[StaticResourceLocation],
) -> ClientArgument {
    let key = usize::try_from(parser.id.0)
        .ok()
        .and_then(|index| parser::ENTRIES.get(index))
        .copied();
    let grammar = VanillaArgument::from_wire(parser, |name| {
        registries
            .iter()
            .copied()
            .find(|registry| registry.as_str() == name)
    });
    ClientArgument {
        grammar,
        suggestions: if grammar.is_some() {
            suggestions
        } else {
            SuggestionType::AskServer
        },
        message: key == Some(parser::MESSAGE),
    }
}

/// The state of a node in the depth-first visit of the child links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visit {
    New,
    OnStack,
    Done,
}

/// Rejects a packet whose child links form a cycle, with a depth-first visit.
fn check_child_cycles(nodes: &[CommandNode<'_>]) -> Result<(), TreeError> {
    let len = nodes.len();
    let mut visits = vec![Visit::New; len];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for start in 0..len {
        if visits.get(start) != Some(&Visit::New) {
            continue;
        }
        stack.push((start, 0));
        if let Some(visit) = visits.get_mut(start) {
            *visit = Visit::OnStack;
        }
        while let Some(&mut (node, ref mut next)) = stack.last_mut() {
            let child = nodes
                .get(node)
                .and_then(|entry| entry.children.get(*next))
                .copied();
            *next += 1;
            match child {
                None => {
                    if let Some(visit) = visits.get_mut(node) {
                        *visit = Visit::Done;
                    }
                    stack.pop();
                }
                Some(child) => {
                    let child = index(child, len)?;
                    match visits.get(child) {
                        Some(Visit::OnStack) => return Err(TreeError::ChildCycle(child)),
                        Some(Visit::New) => {
                            if let Some(visit) = visits.get_mut(child) {
                                *visit = Visit::OnStack;
                            }
                            stack.push((child, 0));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

/// Decodes the tree of a `commands` packet into the client tree. A node that
/// two parents name is one node with two parents. `registries` are the registries whose
/// identifiers the client can name statically; an argument of another registry reads as a
/// type without a grammar.
pub fn client_tree(
    nodes: &[CommandNode<'_>],
    root: VarInt,
    registries: &[StaticResourceLocation],
) -> Result<ClientGraph, TreeError> {
    let len = nodes.len();
    let root = index(root, len)?;
    if !matches!(
        nodes.get(root).map(|node| &node.kind),
        Some(CommandNodeKind::Root)
    ) {
        return Err(TreeError::NotRoot);
    }
    check_child_cycles(nodes)?;
    let mut builder = GraphBuilder::<ClientArgument, (), ()>::new();
    let mut ids: Vec<Option<NodeId>> = vec![None; len];
    if let Some(slot) = ids.get_mut(root) {
        *slot = Some(NodeId::ROOT);
    }
    let mut queue = VecDeque::from([root]);
    while let Some(at) = queue.pop_front() {
        let (Some(node), Some(Some(parent))) = (nodes.get(at), ids.get(at).copied()) else {
            continue;
        };
        for &child in &node.children {
            let child = index(child, len)?;
            if let Some(&Some(existing)) = ids.get(child) {
                builder.link(parent, existing)?;
                continue;
            }
            let Some(entry) = nodes.get(child) else {
                return Err(TreeError::Index(child));
            };
            let id = match &entry.kind {
                CommandNodeKind::Root => return Err(TreeError::NotRoot),
                CommandNodeKind::Literal(name) => builder.literal(parent, name, ())?,
                CommandNodeKind::Argument {
                    name,
                    parser,
                    suggestions,
                } => {
                    let suggestions = suggestions.as_ref().map_or(SuggestionType::OfType, |name| {
                        SuggestionType::from(name.as_str())
                    });
                    builder
                        .argument(parent, name, argument(parser, suggestions, registries), ())?
                        .node()
                }
            };
            if let Some(slot) = ids.get_mut(child) {
                *slot = Some(id);
            }
            queue.push_back(child);
        }
    }
    for (at, node) in nodes.iter().enumerate() {
        let Some(Some(id)) = ids.get(at).copied() else {
            continue;
        };
        if node.executable {
            builder.executes(id, ())?;
        }
        if let Some(target) = node.redirect {
            let target = index(target, len)?;
            let target = ids
                .get(target)
                .copied()
                .flatten()
                .ok_or(TreeError::Unreachable(target))?;
            builder.redirect(id, target)?;
        }
    }
    Ok(builder.freeze()?)
}
