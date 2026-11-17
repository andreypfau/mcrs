use core::fmt;
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_command::{
    Lookup, ResourceError, Source, SpannedIdentifier, identifier_matches,
    prefixed_identifier_matches, read_identifier, suggest_prefixed,
};
use mcrs_minecraft_resource::ResourceLocation;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceLocationArgument;

impl<S> Argument<S> for ResourceLocationArgument {
    type Value<'a> = ResourceLocation<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<ResourceLocation<'a>, SyntaxError<ResourceError>> {
        read_identifier(reader).map(|read| read.identifier)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DimensionArgument;

impl DimensionArgument {
    pub fn suggest<'t, L: Lookup, T>(
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        builder.suggest_matching(source.lookup.dimensions(), identifier_matches);
    }
}

impl<S> Argument<S> for DimensionArgument {
    type Value<'a> = ResourceLocation<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &S,
    ) -> Result<ResourceLocation<'a>, SyntaxError<ResourceError>> {
        ResourceLocationArgument.parse(reader, source)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceOrTag<'a> {
    Entry(ResourceLocation<'a>),
    Tag(ResourceLocation<'a>),
}

impl fmt::Display for ResourceOrTag<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceOrTag::Entry(id) => write!(f, "{id}"),
            ResourceOrTag::Tag(id) => write!(f, "#{id}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceOrTagArgument {
    pub registry: ResourceLocation<'static>,
    pub must_exist: bool,
}

impl ResourceOrTagArgument {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceArgument {
    pub registry: ResourceLocation<'static>,
}

impl ResourceArgument {
    pub fn suggest<'t, L: Lookup, T>(
        &self,
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        builder.suggest_matching(source.lookup.entries(self.registry), identifier_matches);
    }
}

impl<L: Lookup> Argument<Source<'_, L>> for ResourceArgument {
    type Value<'a> = ResourceLocation<'a>;
    type Error = ResourceError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<ResourceLocation<'a>, SyntaxError<ResourceError>> {
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
