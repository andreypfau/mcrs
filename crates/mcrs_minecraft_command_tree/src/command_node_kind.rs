use crate::argument_parser::ArgumentParser;
use mcrs_minecraft_resource::ResourceLocation;
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub enum CommandNodeKind<'a> {
    Root,
    Literal(Cow<'a, str>),
    Argument {
        name: Cow<'a, str>,
        parser: ArgumentParser<'a>,
        suggestions: Option<ResourceLocation<'a>>,
    },
}
