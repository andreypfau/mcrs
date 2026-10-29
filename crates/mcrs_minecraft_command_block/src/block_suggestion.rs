use crate::block_lookup::BlockLookup;
use crate::block_predicate::{BlockPredicateArgument, Stage, entries, read};
use mcrs_command::{
    Reader, StringValue, SuggestionBuilder, SuggestionText, starts_with_ignore_case,
};
use mcrs_minecraft_block::definition::schema::{BlockProperty, PropertyValue};
use mcrs_minecraft_block::keys::BLOCK;
use mcrs_minecraft_command::{Identifier, ResourceOrTag, ResourceOrTagArgument, Source};

const DIGITS: &str = "0123456789";

/// The decimal text of a property value as a suggestion, without allocation.
fn value_text(value: &PropertyValue) -> Option<SuggestionText<'_>> {
    let digit = |n: i32| {
        let n = usize::try_from(n).ok()?;
        DIGITS.get(n..=n)
    };
    match value {
        PropertyValue::Str(name) => Some(SuggestionText::new(name)),
        PropertyValue::Bool(true) => Some(SuggestionText::new("true")),
        PropertyValue::Bool(false) => Some(SuggestionText::new("false")),
        // chisle: integer values from 0 to 99 have a text; every vanilla integer property
        // is in that range. A caller-owned text buffer lifts it.
        PropertyValue::Int(n @ 0..=9) => digit(*n).map(SuggestionText::new),
        PropertyValue::Int(n @ 10..=99) => {
            Some(SuggestionText::concat(digit(n / 10)?, digit(n % 10)?))
        }
        PropertyValue::Int(_) => None,
    }
}

fn named_in(input: &str, list: usize, property: &BlockProperty) -> bool {
    entries(input, Some(list)).any(|(name, _)| StringValue::new(input, name) == *property.name)
}

fn suggest_names<'t, T>(
    builder: &mut SuggestionBuilder<'t, '_, T>,
    list: usize,
    properties: &'t [BlockProperty],
) {
    let input = builder.input();
    let remaining = builder.remaining();
    for property in properties {
        if !named_in(input, list, property) && starts_with_ignore_case(&property.name, remaining) {
            builder.suggest(SuggestionText::concat(&property.name, "="), None);
        }
    }
}

fn suggest_values<'t, T>(builder: &mut SuggestionBuilder<'t, '_, T>, property: &'t BlockProperty) {
    for text in property.values.iter().filter_map(value_text) {
        builder.suggest(text, None);
    }
}

fn has_block_entity<L: BlockLookup>(lookup: &L, subject: ResourceOrTag<'_>) -> bool {
    match subject {
        ResourceOrTag::Entry(block) => lookup.has_block_entity(block),
        ResourceOrTag::Tag(tag) => lookup
            .tag_entries(BLOCK.location(), tag)
            .filter_map(Identifier::parse)
            .any(|member| lookup.has_block_entity(member)),
    }
}

fn if_empty<'t, T>(builder: &mut SuggestionBuilder<'t, '_, T>, text: &'t str) {
    if builder.remaining().is_empty() {
        builder.suggest(SuggestionText::new(text), None);
    }
}

const BLOCK_OR_TAG: ResourceOrTagArgument = ResourceOrTagArgument {
    registry: BLOCK.location(),
    must_exist: true,
};

impl BlockPredicateArgument {
    /// Suggests the block or the tag, its properties and its data (ARG-BLK-013, ARG-BLK-014).
    pub fn suggest<'t, L: BlockLookup, T: Copy>(
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        let lookup: &'t L = source.lookup;
        let registry = BLOCK.location();
        let mut stage = Stage::Start;
        let mut reader = Reader::at(builder.input(), builder.start());
        let point = match read(&mut reader, lookup, &mut stage) {
            Ok(_) => reader.cursor(),
            Err(error) => error.position,
        };
        let input = builder.input();
        let mut builder = builder.at(point);
        let members = |tag: Identifier<'t>| {
            lookup
                .tag_entries(registry, tag)
                .filter_map(Identifier::parse)
                .filter_map(|member| lookup.block_properties(member))
        };
        match stage {
            Stage::Start => {
                BLOCK_OR_TAG.suggest(source, &mut builder);
            }
            Stage::AfterSubject(subject) => {
                let has_properties = match subject {
                    ResourceOrTag::Entry(block) => lookup
                        .block_properties(block)
                        .is_some_and(|properties| !properties.is_empty()),
                    ResourceOrTag::Tag(tag) => {
                        members(tag).any(|properties| !properties.is_empty())
                    }
                };
                if has_properties {
                    if_empty(&mut builder, "[");
                }
                if has_block_entity(lookup, subject) {
                    if_empty(&mut builder, "{");
                }
            }
            Stage::BlockName { block, list, first } => {
                if first {
                    if_empty(&mut builder, "]");
                }
                let properties = lookup.block_properties(block).unwrap_or_default();
                suggest_names(&mut builder, list, properties);
            }
            Stage::BlockEquals => if_empty(&mut builder, "="),
            Stage::BlockValue { block, property } => {
                let property = lookup
                    .block_properties(block)
                    .and_then(|properties| properties.get(property));
                if let Some(property) = property {
                    suggest_values(&mut builder, property);
                }
            }
            Stage::BlockAfterValue { block, list } => {
                if_empty(&mut builder, "]");
                let properties = lookup.block_properties(block).unwrap_or_default();
                if properties
                    .iter()
                    .any(|property| !named_in(input, list, property))
                {
                    if_empty(&mut builder, ",");
                }
            }
            Stage::TagName { tag, list, first } => {
                if first {
                    if_empty(&mut builder, "]");
                }
                for properties in members(tag) {
                    suggest_names(&mut builder, list, properties);
                }
            }
            Stage::TagValue { tag, list, name } => {
                let name = StringValue::new(input, name);
                let mut unnamed = false;
                for properties in members(tag) {
                    for property in properties {
                        if name == *property.name {
                            suggest_values(&mut builder, property);
                        } else if !named_in(input, list, property) {
                            unnamed = true;
                        }
                    }
                }
                if unnamed {
                    builder.suggest(SuggestionText::new(","), None);
                }
                builder.suggest(SuggestionText::new("]"), None);
            }
            Stage::AfterList(subject) => {
                if has_block_entity(lookup, subject) {
                    if_empty(&mut builder, "{");
                }
            }
            Stage::Data => {}
        }
    }
}
