use crate::argument_parser::ArgumentParser;
use mcrs_minecraft_resource::ResourceLocation;
use std::borrow::Cow;

/// What a node of the command tree reads.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandNodeKind<'a> {
    /// The root, which reads nothing.
    Root,
    /// A literal word.
    Literal(Cow<'a, str>),
    /// An argument, with the parser that reads it and the suggestion type that suggests it.
    Argument {
        name: Cow<'a, str>,
        parser: ArgumentParser<'a>,
        suggestions: Option<ResourceLocation<'a>>,
    },
}
