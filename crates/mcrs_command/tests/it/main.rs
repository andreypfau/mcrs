#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

mod alloc_free;
mod exec;
mod parse;
mod reader;
mod suggest;

use mcrs_command::{
    Argument, ArgumentSet, BoolArgument, Contains, Graph, GraphBuilder, IntegerArgument, NodeId,
    Reader, Requirement, StringArgument, SuggestionBuilder, SyntaxError,
};
use std::convert::Infallible;

/// The source is a permission level; a node requires one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level(pub u8);

impl Requirement<u8> for Level {
    fn test(&self, source: &u8) -> bool {
        *source >= self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lib {
    Bool(BoolArgument),
    Integer(IntegerArgument),
    String(StringArgument),
}

impl From<IntegerArgument> for Lib {
    fn from(value: IntegerArgument) -> Self {
        Lib::Integer(value)
    }
}

impl From<StringArgument> for Lib {
    fn from(value: StringArgument) -> Self {
        Lib::String(value)
    }
}

impl From<BoolArgument> for Lib {
    fn from(value: BoolArgument) -> Self {
        Lib::Bool(value)
    }
}

impl Contains<IntegerArgument> for Lib {
    fn get(&self) -> Option<&IntegerArgument> {
        match self {
            Lib::Integer(argument) => Some(argument),
            _ => None,
        }
    }
}

impl Contains<StringArgument> for Lib {
    fn get(&self) -> Option<&StringArgument> {
        match self {
            Lib::String(argument) => Some(argument),
            _ => None,
        }
    }
}

impl ArgumentSet<u8> for Lib {
    type Error = Infallible;
    type Tooltip = ();

    fn check(&self, reader: &mut Reader<'_>, source: &u8) -> Result<(), SyntaxError<Infallible>> {
        match self {
            Lib::Bool(argument) => argument.parse(reader, source).map(drop),
            Lib::Integer(argument) => argument.parse(reader, source).map(drop),
            Lib::String(argument) => argument.parse(reader, source).map(drop),
        }
    }

    fn suggest<'t>(
        &'t self,
        _: &'t u8,
        _: &mcrs_command::ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, ()>,
    ) {
        if let Lib::Bool(_) = self {
            BoolArgument::suggest(builder);
        }
    }
}

pub type TestGraph = Graph<Lib, Level, u32>;
pub type Builder = GraphBuilder<Lib, Level, u32>;

pub const OPEN: Level = Level(0);
pub const OP: Level = Level(2);

pub fn executable_literal(builder: &mut Builder, parent: NodeId, name: &str) -> NodeId {
    let node = builder.literal(parent, name, OPEN).unwrap();
    builder.executes(node, 0).unwrap();
    node
}
