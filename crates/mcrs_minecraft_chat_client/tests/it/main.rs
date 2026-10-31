#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

mod commands;
mod log;
mod screen;

use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_chat_client::{ChatAbilities, Context};
use mcrs_minecraft_command::{CommandLevel, Lookup, Source};
use mcrs_minecraft_command_argument::TimeLookup;
use mcrs_minecraft_command_argument_block::BlockLookup;
use mcrs_minecraft_command_argument_vanilla::{ClientGraph, client_tree};
use mcrs_minecraft_command_tree::{
    ArgumentParser, CommandNode, CommandNodeKind, ParserProperties, StringMode,
};
use mcrs_minecraft_keys::command_argument_type as parser;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundCommands;
use mcrs_minecraft_resource::ResourceLocation;
use std::borrow::Cow;

/// The players online, and nothing else.
pub struct Players(pub &'static [&'static str]);

impl Lookup for Players {
    fn entries(&self, _: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        std::iter::empty()
    }

    fn tags(&self, _: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        std::iter::empty()
    }

    fn tag_contains(
        &self,
        _: ResourceLocation<'static>,
        _: ResourceLocation<'_>,
        _: ResourceLocation<'static>,
    ) -> bool {
        false
    }

    fn tag_entries(
        &self,
        _: ResourceLocation<'static>,
        _: ResourceLocation<'_>,
    ) -> impl Iterator<Item = &str> {
        std::iter::empty()
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().copied()
    }

    fn dimensions(&self) -> impl Iterator<Item = &str> {
        std::iter::empty()
    }
}

impl BlockLookup for Players {
    fn block_properties(&self, _: ResourceLocation<'_>) -> Option<&[BlockProperty]> {
        None
    }

    fn has_block_entity(&self, _: ResourceLocation<'_>) -> bool {
        false
    }
}

impl TimeLookup for Players {
    fn suggested_time_markers(
        &self,
        _: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str> {
        std::iter::empty()
    }

    fn timelines_of_clock(&self, _: Option<ResourceLocation<'_>>) -> impl Iterator<Item = &str> {
        std::iter::empty()
    }
}

pub static PLAYERS: Players = Players(&["Alex", "Steve"]);

pub fn source() -> Source<'static, Players> {
    Source::new(CommandLevel::Gamemasters, &PLAYERS)
}

pub fn context<'a>(tree: &'a ClientGraph, source: &'a Source<'a, Players>) -> Context<'a, Players> {
    Context {
        tree,
        source,
        abilities: ChatAbilities::default(),
        auto_suggest: true,
        completions: &[],
    }
}

/// A node of a `commands` packet under construction.
pub enum Wire {
    Literal(&'static str),
    Argument(
        &'static str,
        ResourceLocation<'static>,
        ParserProperties<'static>,
        Option<&'static str>,
    ),
}

pub struct Tree(Vec<CommandNode<'static>>);

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    pub fn new() -> Self {
        Tree(vec![CommandNode {
            kind: CommandNodeKind::Root,
            children: Vec::new(),
            redirect: None,
            executable: false,
            restricted: false,
        }])
    }

    pub fn add(&mut self, parent: usize, node: Wire, executable: bool) -> usize {
        let kind = match node {
            Wire::Literal(name) => CommandNodeKind::Literal(Cow::Borrowed(name)),
            Wire::Argument(name, key, properties, suggestions) => CommandNodeKind::Argument {
                name: Cow::Borrowed(name),
                parser: ArgumentParser {
                    id: VarInt(
                        parser::ENTRIES
                            .iter()
                            .position(|&entry| entry == key)
                            .unwrap() as i32,
                    ),
                    properties,
                },
                suggestions: suggestions.map(|name| ResourceLocation::read(name).unwrap()),
            },
        };
        let index = self.0.len();
        self.0.push(CommandNode {
            kind,
            children: Vec::new(),
            redirect: None,
            executable,
            restricted: false,
        });
        self.0[parent].children.push(VarInt(index as i32));
        index
    }

    pub fn redirect(&mut self, node: usize, target: usize) {
        self.0[node].redirect = Some(VarInt(target as i32));
    }

    pub fn build(self) -> ClientGraph {
        let packet = ClientboundCommands {
            nodes: self.0,
            root: VarInt(0),
        };
        client_tree(&packet.nodes, packet.root, &[]).unwrap()
    }
}

pub fn greedy() -> ParserProperties<'static> {
    ParserProperties::String(StringMode::Greedy)
}
