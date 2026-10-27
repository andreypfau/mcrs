use crate::error::CommandSyntaxError;
use crate::identifier::identifier_matches;
use crate::set::VanillaArgument;
use crate::source::{Lookup, Source};
use crate::{
    BlockPosArgument, BlockPredicateArgument, DimensionArgument, EntityAnchorArgument,
    EntityArgument, GameModeArgument, HeightmapArgument, ResourceLocationArgument,
    ResourceOrTagArgument, RotationArgument, SwizzleArgument, Vec3Argument,
};
use mcrs_command::{
    ArgumentSet, BoolArgument, BuildError, DoubleArgument, FloatArgument, Graph, GraphBuilder,
    IntegerArgument, LongArgument, NodeId, Reader, Requirement, StringArgument, SuggestionBuilder,
};
use mcrs_minecraft_block::keys::BLOCK;
use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_dimension::keys::DIMENSION;
use mcrs_minecraft_keys::command_argument_type as parser;
use mcrs_minecraft_protocol::command::{
    ArgumentParser, CommandNodeKind, NumberRange, ParserProperties, StringMode,
};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundCommands;

const SOUND_EVENT: StaticResourceLocation = mcrs_minecraft_core::rl!("minecraft:sound_event");

/// The client tree of the last `commands` packet (CCMD-TREE-001). An executable node runs
/// nothing on the client, so its command is `()`.
pub type ClientGraph = Graph<ClientArgument, ClientRequirement, ()>;

/// The client source holds the restricted atom, so every node passes (CCMD-TREE-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientRequirement {
    pub restricted: bool,
}

impl<S> Requirement<S> for ClientRequirement {
    fn test(&self, _: &S) -> bool {
        true
    }
}

/// Who makes the suggestions of an argument (CMDP-ARG-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionType {
    OfType,
    AskServer,
    AvailableSounds,
    SummonableEntities,
    PostEffects,
}

impl SuggestionType {
    fn of(name: &str) -> Self {
        match name {
            "minecraft:available_sounds" => SuggestionType::AvailableSounds,
            "minecraft:summonable_entities" => SuggestionType::SummonableEntities,
            "minecraft:post_effects" => SuggestionType::PostEffects,
            _ => SuggestionType::AskServer,
        }
    }
}

/// An argument of the client tree: the grammar of its parser id, or `None` when the client
/// lacks that grammar and reads the rest of the line (CCMD-TREE-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientArgument {
    pub grammar: Option<VanillaArgument>,
    pub suggestions: SuggestionType,
    /// The parser id is `minecraft:message` (CCMD-PARSE-002).
    pub message: bool,
}

impl ClientArgument {
    /// The argument takes its suggestions from a server request (CCMD-SUG-002).
    pub fn needs_server<L: Lookup>(&self, lookup: &L) -> bool {
        let Some(grammar) = self.grammar else {
            return true;
        };
        match self.suggestions {
            SuggestionType::AskServer
            | SuggestionType::SummonableEntities
            | SuggestionType::PostEffects => return true,
            SuggestionType::AvailableSounds => return !lookup.has_registry(SOUND_EVENT),
            SuggestionType::OfType => {}
        }
        match grammar {
            VanillaArgument::Dimension(_) => !lookup.has_registry(DIMENSION.location()),
            VanillaArgument::ResourceOrTag(argument) => !lookup.has_registry(argument.registry),
            VanillaArgument::BlockPredicate(_) => !lookup.has_registry(BLOCK.location()),
            _ => false,
        }
    }
}

impl<L: Lookup> ArgumentSet<Source<'_, L>> for ClientArgument {
    type Error = crate::ArgumentError;
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
        builder: &mut SuggestionBuilder<'t, '_, &'static str>,
    ) {
        match (&self.grammar, self.suggestions) {
            (Some(grammar), SuggestionType::OfType) => grammar.suggest(source, builder),
            (Some(_), SuggestionType::AvailableSounds) => {
                builder.suggest_matching(source.lookup.entries(SOUND_EVENT), identifier_matches);
            }
            _ => {}
        }
    }
}

/// Why the client rejects a `commands` packet (CMDP-PKT-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeError {
    /// A child or redirect index, or the root index, is out of range.
    Index(usize),
    /// The root index does not name a root node.
    NotRoot,
    /// The child links of this node lead back to it.
    ChildCycle(usize),
    /// A redirect target that no child link reaches.
    Unreachable(usize),
    Build(BuildError),
}

impl From<BuildError> for TreeError {
    fn from(error: BuildError) -> Self {
        TreeError::Build(error)
    }
}

impl core::fmt::Display for TreeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

fn index(value: mcrs_minecraft_protocol::VarInt, len: usize) -> Result<usize, TreeError> {
    usize::try_from(value.0)
        .ok()
        .filter(|&index| index < len)
        .ok_or(TreeError::Index(value.0.unsigned_abs() as usize))
}

fn number<T>(range: &NumberRange<T>, any_min: T, any_max: T) -> (T, T)
where
    T: Copy,
{
    (range.min.unwrap_or(any_min), range.max.unwrap_or(any_max))
}

/// The grammar of a parser id and its properties, or `None` when the client has none for it.
/// `registries` are the registries whose identifiers the client can name statically.
fn grammar(
    parser: &ArgumentParser<'_>,
    registries: &[StaticResourceLocation],
) -> Option<VanillaArgument> {
    let key = usize::try_from(parser.id.0)
        .ok()
        .and_then(|index| parser::ENTRIES.get(index))?;
    let registry = |name: &str| {
        registries
            .iter()
            .copied()
            .find(|registry| registry.as_str() == name)
    };
    let argument = match (*key, &parser.properties) {
        (k, ParserProperties::None) if k == parser::BRIGADIER_BOOL => BoolArgument.into(),
        (k, ParserProperties::Integer(range)) if k == parser::BRIGADIER_INTEGER => {
            let (min, max) = number(range, i32::MIN, i32::MAX);
            IntegerArgument { min, max }.into()
        }
        (k, ParserProperties::Long(range)) if k == parser::BRIGADIER_LONG => {
            let (min, max) = number(range, i64::MIN, i64::MAX);
            LongArgument { min, max }.into()
        }
        (k, ParserProperties::Float(range)) if k == parser::BRIGADIER_FLOAT => {
            let (min, max) = number(range, f32::MIN, f32::MAX);
            FloatArgument { min, max }.into()
        }
        (k, ParserProperties::Double(range)) if k == parser::BRIGADIER_DOUBLE => {
            let (min, max) = number(range, f64::MIN, f64::MAX);
            DoubleArgument { min, max }.into()
        }
        (k, ParserProperties::String(mode)) if k == parser::BRIGADIER_STRING => match mode {
            StringMode::Word => StringArgument::Word,
            StringMode::Phrase => StringArgument::Phrase,
            StringMode::Greedy => StringArgument::Greedy,
        }
        .into(),
        (
            k,
            &ParserProperties::Entity {
                single,
                players_only,
            },
        ) if k == parser::ENTITY => EntityArgument {
            single,
            players_only,
        }
        .into(),
        (k, ParserProperties::Registry(name))
            if k == parser::RESOURCE_OR_TAG || k == parser::RESOURCE_OR_TAG_KEY =>
        {
            ResourceOrTagArgument {
                registry: registry(name.as_str())?,
                must_exist: k == parser::RESOURCE_OR_TAG,
            }
            .into()
        }
        (k, ParserProperties::None) => {
            if k == parser::VEC3 {
                Vec3Argument.into()
            } else if k == parser::ROTATION {
                RotationArgument.into()
            } else if k == parser::RESOURCE_LOCATION {
                ResourceLocationArgument.into()
            } else if k == parser::ENTITY_ANCHOR {
                EntityAnchorArgument.into()
            } else if k == parser::DIMENSION {
                DimensionArgument.into()
            } else if k == parser::GAMEMODE {
                GameModeArgument.into()
            } else if k == parser::BLOCK_POS {
                BlockPosArgument.into()
            } else if k == parser::SWIZZLE {
                SwizzleArgument.into()
            } else if k == parser::HEIGHTMAP {
                HeightmapArgument.into()
            } else if k == parser::BLOCK_PREDICATE {
                BlockPredicateArgument.into()
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(argument)
}

fn argument(
    parser: &ArgumentParser<'_>,
    suggestions: SuggestionType,
    registries: &[StaticResourceLocation],
) -> ClientArgument {
    let grammar = grammar(parser, registries);
    ClientArgument {
        grammar,
        suggestions: if grammar.is_some() {
            suggestions
        } else {
            SuggestionType::AskServer
        },
        message: usize::try_from(parser.id.0)
            .ok()
            .and_then(|index| parser::ENTRIES.get(index))
            .is_some_and(|&key| key == parser::MESSAGE),
    }
}

/// Rejects a packet whose child links form a cycle, with a depth-first visit that colours each
/// node (0 new, 1 on the stack, 2 done).
fn check_child_cycles(packet: &ClientboundCommands<'_>) -> Result<(), TreeError> {
    let len = packet.nodes.len();
    let mut colour = vec![0u8; len];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for start in 0..len {
        if colour.get(start) != Some(&0) {
            continue;
        }
        stack.push((start, 0));
        if let Some(c) = colour.get_mut(start) {
            *c = 1;
        }
        while let Some(&mut (node, ref mut next)) = stack.last_mut() {
            let child = packet
                .nodes
                .get(node)
                .and_then(|entry| entry.children.get(*next))
                .copied();
            *next += 1;
            match child {
                None => {
                    if let Some(c) = colour.get_mut(node) {
                        *c = 2;
                    }
                    stack.pop();
                }
                Some(child) => {
                    let child = index(child, len)?;
                    match colour.get(child) {
                        Some(1) => return Err(TreeError::ChildCycle(child)),
                        Some(0) => {
                            if let Some(c) = colour.get_mut(child) {
                                *c = 1;
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

/// Decodes the tree of a `commands` packet into the client tree (CCMD-TREE-001). A node that
/// two parents name is one node with two parents. `registries` are the registries whose
/// identifiers the client can name statically; an argument of another registry reads as a
/// type without a grammar.
pub fn client_tree(
    packet: &ClientboundCommands<'_>,
    registries: &[StaticResourceLocation],
) -> Result<ClientGraph, TreeError> {
    let len = packet.nodes.len();
    let root = index(packet.root, len)?;
    if !matches!(
        packet.nodes.get(root).map(|node| &node.kind),
        Some(CommandNodeKind::Root)
    ) {
        return Err(TreeError::NotRoot);
    }
    check_child_cycles(packet)?;
    let mut builder = GraphBuilder::<ClientArgument, ClientRequirement, ()>::new();
    let mut ids: Vec<Option<NodeId>> = vec![None; len];
    if let Some(slot) = ids.get_mut(root) {
        *slot = Some(NodeId::ROOT);
    }
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(at) = queue.pop_front() {
        let (Some(node), Some(Some(parent))) = (packet.nodes.get(at), ids.get(at).copied()) else {
            continue;
        };
        for &child in &node.children {
            let child = index(child, len)?;
            if let Some(&Some(existing)) = ids.get(child) {
                builder.link(parent, existing)?;
                continue;
            }
            let Some(entry) = packet.nodes.get(child) else {
                return Err(TreeError::Index(child));
            };
            let requirement = ClientRequirement {
                restricted: entry.restricted,
            };
            let id = match &entry.kind {
                CommandNodeKind::Root => return Err(TreeError::NotRoot),
                CommandNodeKind::Literal(name) => builder.literal(parent, name, requirement)?,
                CommandNodeKind::Argument {
                    name,
                    parser,
                    suggestions,
                } => {
                    let suggestions = suggestions.as_ref().map_or(SuggestionType::OfType, |name| {
                        SuggestionType::of(name.as_str())
                    });
                    builder
                        .argument(
                            parent,
                            name,
                            argument(parser, suggestions, registries),
                            requirement,
                        )?
                        .node()
                }
            };
            if let Some(slot) = ids.get_mut(child) {
                *slot = Some(id);
            }
            queue.push_back(child);
        }
    }
    for (at, node) in packet.nodes.iter().enumerate() {
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
