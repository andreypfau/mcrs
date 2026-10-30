use core::fmt;
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_command::{
    Identifier, Lookup, ResourceError, Source, SpannedIdentifier, identifier_matches,
    prefixed_identifier_matches, read_identifier, suggest_prefixed,
};
use mcrs_minecraft_core::StaticResourceLocation;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceLocationArgument;

impl<S> Argument<S> for ResourceLocationArgument {
    type Value<'a> = Identifier<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<Identifier<'a>, SyntaxError<ResourceError>> {
        read_identifier(reader).map(|read| read.identifier)
    }
}

/// `minecraft:dimension`. Whether the dimension exists is the command's test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DimensionArgument;

impl DimensionArgument {
    /// Suggests the dimensions that the source knows.
    pub fn suggest<'t, L: Lookup, T>(
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        builder.suggest_matching(source.lookup.dimensions(), identifier_matches);
    }
}

impl<S> Argument<S> for DimensionArgument {
    type Value<'a> = Identifier<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &S,
    ) -> Result<Identifier<'a>, SyntaxError<ResourceError>> {
        ResourceLocationArgument.parse(reader, source)
    }
}

/// An entry of a registry, or a tag of it written with a leading `#`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceOrTag<'a> {
    /// One entry of the registry.
    Entry(Identifier<'a>),
    /// A tag of the registry.
    Tag(Identifier<'a>),
}

impl fmt::Display for ResourceOrTag<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceOrTag::Entry(id) => write!(f, "{id}"),
            ResourceOrTag::Tag(id) => write!(f, "#{id}"),
        }
    }
}

/// `minecraft:resource_or_tag` when `must_exist`, else
/// `minecraft:resource_or_tag_key`, whose read does not test that the entry or
/// the tag exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceOrTagArgument {
    pub registry: StaticResourceLocation,
    pub must_exist: bool,
}

impl ResourceOrTagArgument {
    /// Suggests the tags of the registry, then its entries.
    pub fn suggest<'t, L: Lookup, T: Copy>(
        &self,
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        let tags = source.lookup.tags(self.registry).map(|tag| (tag, None));
        suggest_prefixed(builder, &["#"], tags, prefixed_identifier_matches);
        builder.suggest_matching(source.lookup.entries(self.registry), identifier_matches);
    }
}

impl<L: Lookup> Argument<Source<'_, L>> for ResourceOrTagArgument {
    type Value<'a> = ResourceOrTag<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<ResourceOrTag<'a>, SyntaxError<ResourceError>> {
        let tag = reader.peek_is('#');
        if tag {
            reader.skip();
        }
        let SpannedIdentifier { identifier, span } = read_identifier(reader)?;
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
                ResourceError::ResourceNotFound { span, registry },
            ),
            ResourceOrTag::Tag(tag) => (
                source.lookup.has_tag(registry, tag),
                ResourceError::ResourceTagNotFound { span, registry },
            ),
        };
        if exists {
            Ok(value)
        } else {
            Err(SyntaxError::argument(error, reader.cursor()))
        }
    }
}

/// `minecraft:resource`: an entry of the registry, which must exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceArgument {
    pub registry: StaticResourceLocation,
}

impl ResourceArgument {
    /// Suggests the entries of the registry.
    pub fn suggest<'t, L: Lookup, T>(
        &self,
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        builder.suggest_matching(source.lookup.entries(self.registry), identifier_matches);
    }
}

impl<L: Lookup> Argument<Source<'_, L>> for ResourceArgument {
    type Value<'a> = Identifier<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<Identifier<'a>, SyntaxError<ResourceError>> {
        let SpannedIdentifier { identifier, span } = read_identifier(reader)?;
        if source.lookup.has_entry(self.registry, identifier) {
            Ok(identifier)
        } else {
            Err(SyntaxError::argument(
                ResourceError::ResourceNotFound {
                    span,
                    registry: self.registry,
                },
                reader.cursor(),
            ))
        }
    }
}
