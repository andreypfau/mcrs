use crate::error::{ArgumentError, CommandSyntaxError};
use crate::identifier::{
    Identifier, identifier_matches, prefixed_identifier_matches, read_identifier, suggest_prefixed,
};
use crate::source::{Lookup, Source};
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_dimension::keys::DIMENSION;

/// `minecraft:resource_location` (ARG-RES-003).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceLocationArgument;

impl<S> Argument<S> for ResourceLocationArgument {
    type Value<'a> = Identifier<'a>;
    type Error = ArgumentError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<Identifier<'a>, CommandSyntaxError> {
        read_identifier(reader).map(|(identifier, _)| identifier)
    }
}

/// `minecraft:dimension` (ARG-RES-004). Whether the dimension exists is the command's test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DimensionArgument;

impl DimensionArgument {
    pub fn suggest<'t, L: Lookup, T>(
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        let dimensions = source.lookup.entries(DIMENSION.location());
        builder.suggest_matching(dimensions, identifier_matches);
    }
}

impl<S> Argument<S> for DimensionArgument {
    type Value<'a> = Identifier<'a>;
    type Error = ArgumentError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &S,
    ) -> Result<Identifier<'a>, CommandSyntaxError> {
        ResourceLocationArgument.parse(reader, source)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceOrTag<'a> {
    Entry(Identifier<'a>),
    Tag(Identifier<'a>),
}

impl core::fmt::Display for ResourceOrTag<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ResourceOrTag::Entry(id) => write!(f, "{id}"),
            ResourceOrTag::Tag(id) => write!(f, "#{id}"),
        }
    }
}

/// `minecraft:resource_or_tag` (ARG-RES-006) when `must_exist`, else
/// `minecraft:resource_or_tag_key` (ARG-RES-005), whose read does not test that the entry or
/// the tag exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceOrTagArgument {
    pub registry: StaticResourceLocation,
    pub must_exist: bool,
}

impl ResourceOrTagArgument {
    pub fn suggest<'t, L: Lookup, T: Copy>(
        &self,
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        let remaining = builder.remaining();
        let tags = source.lookup.tags(self.registry).map(|tag| (tag, None));
        suggest_prefixed(
            builder,
            &["#"],
            tags,
            remaining,
            prefixed_identifier_matches,
        );
        builder.suggest_matching(source.lookup.entries(self.registry), identifier_matches);
    }
}

impl<L: Lookup> Argument<Source<'_, L>> for ResourceOrTagArgument {
    type Value<'a> = ResourceOrTag<'a>;
    type Error = ArgumentError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<ResourceOrTag<'a>, CommandSyntaxError> {
        let tag = reader.peek_is('#');
        if tag {
            reader.skip();
        }
        let (identifier, id) = read_identifier(reader)?;
        let value = if tag {
            ResourceOrTag::Tag(identifier)
        } else {
            ResourceOrTag::Entry(identifier)
        };
        if !self.must_exist {
            return Ok(value);
        }
        let registry = self.registry;
        let (exists, error) = match value {
            ResourceOrTag::Entry(entry) => (
                source.lookup.has_entry(registry, entry),
                ArgumentError::ResourceNotFound { id, registry },
            ),
            ResourceOrTag::Tag(tag) => (
                source.lookup.has_tag(registry, tag),
                ArgumentError::ResourceTagNotFound { id, registry },
            ),
        };
        if exists {
            Ok(value)
        } else {
            Err(SyntaxError::argument(error, reader.cursor()))
        }
    }
}
