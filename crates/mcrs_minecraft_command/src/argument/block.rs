use crate::error::{ArgumentError, BlockSubject, CommandSyntaxError};
use crate::identifier::{
    Identifier, identifier_matches, prefixed_identifier_matches, read_identifier, suggest_prefixed,
};
use crate::source::{Lookup, Source};
use mcrs_command::suggest::starts_with_ignore_case;
use mcrs_command::{
    Argument, Reader, Span, StringSpan, StringValue, SuggestionBuilder, SuggestionText, SyntaxError,
};
use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::definition::schema::{BlockProperty, PropertyValue};
use mcrs_minecraft_block::keys::BLOCK;
use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::snbt::{check_compound, read_compound};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockOrTag<'a> {
    Block(Identifier<'a>),
    Tag(Identifier<'a>),
}

/// The value of a `block_predicate`. The property list and the data compound stay text and are
/// read again by the test, so the value borrows the command line and owns nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockPredicate<'a> {
    pub subject: BlockOrTag<'a>,
    input: &'a str,
    /// The position of the `[` of the property list.
    list: Option<usize>,
    /// The position of the `{` of the data compound.
    data: Option<usize>,
}

impl<'a> BlockPredicate<'a> {
    /// The pairs of a name and a value text of the property list, in their order.
    fn properties(&self) -> impl Iterator<Item = (StringValue<'a>, StringValue<'a>)> + 'a {
        let input = self.input;
        entries(input, self.list).map(move |(name, value)| {
            (
                StringValue::new(input, name),
                StringValue::new(input, value),
            )
        })
    }

    /// ARG-BLK-011: without a data compound the test passes. With one, `saved` gives the saved
    /// data of the block entity at the position (ARG-BLK-012), or `None` when there is none. The
    /// compound (ARG-BLK-007) is built from the text the read validated.
    pub fn test_data(&self, saved: impl FnOnce() -> Option<NbtCompound>) -> bool {
        let Some(start) = self.data else {
            return true;
        };
        let Ok((expected, _)) = read_compound(self.input, start) else {
            return false;
        };
        saved().is_some_and(|actual| expected.matches(&actual))
    }

    /// The property test of a block state (ARG-BLK-010). The caller tests that the block of the
    /// state is the block of the predicate, or a member of its tag (ARG-BLK-009).
    pub fn test(&self, block: &BlockEntry, state: BlockStateId) -> bool {
        self.properties().all(|(name, text)| {
            block
                .properties
                .0
                .iter()
                .find(|property| name.eq_str(&property.name))
                .and_then(|property| block.value_of(state, &property.name))
                .is_some_and(|value| value_text_is(value, text))
        })
    }
}

/// Reads an optional sign and ASCII digits as a 32-bit integer (ARG-BLK-005). A text with an
/// escape holds a backslash, so it is no integer.
fn integer(text: StringValue<'_>) -> Option<i32> {
    text.as_unescaped()?.parse().ok()
}

/// Whether the value text names `value` (ARG-BLK-005).
fn value_text_is(value: &PropertyValue, text: StringValue<'_>) -> bool {
    match value {
        PropertyValue::Str(name) => text.eq_str(name),
        PropertyValue::Bool(true) => text.eq_str("true"),
        PropertyValue::Bool(false) => text.eq_str("false"),
        PropertyValue::Int(number) => integer(text) == Some(*number),
    }
}

/// The pairs of an already validated property list.
fn entries(
    input: &str,
    list: Option<usize>,
) -> impl Iterator<Item = (StringSpan, StringSpan)> + '_ {
    let mut reader = list.map(|start| {
        let mut reader = Reader::at(input, start);
        reader.skip();
        reader
    });
    core::iter::from_fn(move || {
        let reader = reader.as_mut()?;
        reader.skip_whitespace();
        if reader.at_end() || reader.peek_is(']') {
            return None;
        }
        let name = reader.read_string::<ArgumentError>().ok()?;
        reader.skip_whitespace();
        if !reader.peek_is('=') {
            return None;
        }
        reader.skip();
        reader.skip_whitespace();
        let value = reader.read_string::<ArgumentError>().ok()?;
        reader.skip_whitespace();
        if reader.peek_is(',') {
            reader.skip();
        }
        Some((name, value))
    })
}

/// Where a read stopped, for the suggestions (ARG-BLK-014).
#[derive(Clone, Copy, Debug, Default)]
enum Stage<'a> {
    #[default]
    Start,
    AfterBlock(Identifier<'a>),
    AfterTag(Identifier<'a>),
    BlockName {
        block: Identifier<'a>,
        list: usize,
        first: bool,
    },
    BlockEquals,
    BlockValue {
        block: Identifier<'a>,
        property: usize,
    },
    BlockAfterValue {
        block: Identifier<'a>,
        list: usize,
    },
    TagName {
        tag: Identifier<'a>,
        list: usize,
        first: bool,
    },
    TagValue {
        tag: Identifier<'a>,
        list: usize,
        name: StringSpan,
    },
    AfterList(BlockOrTag<'a>),
    Data,
}

/// `minecraft:block_predicate` (ARG-BLK-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockPredicateArgument;

/// One entry of the property list of a block (ARG-BLK-004, step 3).
fn block_entry<'a>(
    reader: &mut Reader<'a>,
    properties: &[BlockProperty],
    block: (Identifier<'a>, Span),
    named: &mut u64,
    stage: &mut Stage<'a>,
) -> Result<(), CommandSyntaxError> {
    let (id, block_span) = block;
    reader.skip_whitespace();
    let name_start = reader.cursor();
    let name = reader.read_string()?;
    let name_text = StringValue::new(reader.input(), name);
    let Some(index) = properties
        .iter()
        .position(|property| name_text.eq_str(&property.name))
    else {
        reader.set_cursor(name_start);
        return Err(SyntaxError::argument(
            ArgumentError::PropertyUnknown {
                block: block_span,
                name,
            },
            name_start,
        ));
    };
    let bit = 1u64
        .checked_shl(u32::try_from(index).unwrap_or(u32::MAX))
        .unwrap_or(0);
    if *named & bit != 0 {
        reader.set_cursor(name_start);
        return Err(SyntaxError::argument(
            ArgumentError::PropertyDuplicate {
                name,
                subject: BlockSubject::block(block_span),
            },
            name_start,
        ));
    }
    *named |= bit;
    reader.skip_whitespace();
    *stage = Stage::BlockEquals;
    if reader.at_end() || !reader.peek_is('=') {
        return Err(SyntaxError::argument(
            ArgumentError::PropertyNoValue {
                name,
                subject: BlockSubject::block(block_span),
            },
            reader.cursor(),
        ));
    }
    reader.skip();
    reader.skip_whitespace();
    *stage = Stage::BlockValue {
        block: id,
        property: index,
    };
    let value_start = reader.cursor();
    let value = reader.read_string()?;
    let valid = properties.get(index).is_some_and(|property| {
        property
            .values
            .iter()
            .any(|known| value_text_is(known, StringValue::new(reader.input(), value)))
    });
    if !valid {
        reader.set_cursor(value_start);
        return Err(SyntaxError::argument(
            ArgumentError::PropertyInvalid {
                block: block_span,
                value,
                name,
            },
            value_start,
        ));
    }
    Ok(())
}

fn block_list<'a, L: Lookup>(
    reader: &mut Reader<'a>,
    lookup: &L,
    block: (Identifier<'a>, Span),
    stage: &mut Stage<'a>,
) -> Result<(), CommandSyntaxError> {
    let (id, block_span) = block;
    let properties = lookup.block_properties(id).unwrap_or_default();
    let list = reader.cursor();
    reader.skip();
    reader.skip_whitespace();
    let mut named: u64 = 0;
    let mut first = true;
    loop {
        *stage = Stage::BlockName {
            block: id,
            list,
            first,
        };
        if reader.peek_is(']') || reader.at_end() {
            break;
        }
        block_entry(reader, properties, (id, block_span), &mut named, stage)?;
        *stage = Stage::BlockAfterValue { block: id, list };
        reader.skip_whitespace();
        if reader.at_end() || reader.peek_is(']') {
            break;
        }
        if !reader.peek_is(',') {
            return Err(SyntaxError::argument(
                ArgumentError::PropertyUnclosed,
                reader.cursor(),
            ));
        }
        reader.skip();
        first = false;
    }
    if reader.at_end() {
        return Err(SyntaxError::argument(
            ArgumentError::PropertyUnclosed,
            reader.cursor(),
        ));
    }
    reader.skip();
    *stage = Stage::AfterList(BlockOrTag::Block(id));
    Ok(())
}

fn tag_list<'a>(
    reader: &mut Reader<'a>,
    tag: (Identifier<'a>, Span),
    stage: &mut Stage<'a>,
) -> Result<(), CommandSyntaxError> {
    let (id, tag_span) = tag;
    let list = reader.cursor();
    reader.skip();
    reader.skip_whitespace();
    let mut value_start = None;
    let mut first = true;
    loop {
        *stage = Stage::TagName {
            tag: id,
            list,
            first,
        };
        if reader.peek_is(']') || reader.at_end() {
            break;
        }
        reader.skip_whitespace();
        let name_start = reader.cursor();
        let name = reader.read_string()?;
        let name_text = StringValue::new(reader.input(), name);
        let duplicate = entries(reader.input(), Some(list))
            .take_while(|(earlier, _)| earlier.span.start < name_start)
            .any(|(earlier, _)| {
                StringValue::new(reader.input(), earlier)
                    .chars()
                    .eq(name_text.chars())
            });
        let subject = BlockSubject::tag(tag_span);
        if duplicate {
            reader.set_cursor(name_start);
            return Err(SyntaxError::argument(
                ArgumentError::PropertyDuplicate { name, subject },
                name_start,
            ));
        }
        reader.skip_whitespace();
        if reader.at_end() || !reader.peek_is('=') {
            reader.set_cursor(name_start);
            return Err(SyntaxError::argument(
                ArgumentError::PropertyNoValue { name, subject },
                name_start,
            ));
        }
        reader.skip();
        reader.skip_whitespace();
        *stage = Stage::TagValue {
            tag: id,
            list,
            name,
        };
        value_start = Some(reader.cursor());
        reader.read_string()?;
        reader.skip_whitespace();
        if reader.at_end() {
            break;
        }
        value_start = None;
        if reader.peek_is(']') {
            break;
        }
        if !reader.peek_is(',') {
            return Err(SyntaxError::argument(
                ArgumentError::PropertyUnclosed,
                reader.cursor(),
            ));
        }
        reader.skip();
        first = false;
    }
    if reader.at_end() {
        if let Some(start) = value_start {
            reader.set_cursor(start);
        }
        return Err(SyntaxError::argument(
            ArgumentError::PropertyUnclosed,
            reader.cursor(),
        ));
    }
    reader.skip();
    *stage = Stage::AfterList(BlockOrTag::Tag(id));
    Ok(())
}

fn read<'a, L: Lookup>(
    reader: &mut Reader<'a>,
    lookup: &L,
    stage: &mut Stage<'a>,
) -> Result<BlockPredicate<'a>, CommandSyntaxError> {
    let start = reader.cursor();
    let registry = BLOCK.location();
    let (subject, list) = if reader.peek_is('#') {
        reader.skip();
        let (id, span) = read_identifier(reader)?;
        if !lookup.has_tag(registry, id) {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(
                ArgumentError::BlockTagUnknown(span),
                start,
            ));
        }
        *stage = Stage::AfterTag(id);
        let list = reader.peek_is('[').then(|| reader.cursor());
        if list.is_some() {
            tag_list(reader, (id, span), stage)?;
        }
        (BlockOrTag::Tag(id), list)
    } else {
        let (id, span) = read_identifier(reader)?;
        if !lookup.has_entry(registry, id) {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(
                ArgumentError::BlockIdInvalid(span),
                start,
            ));
        }
        *stage = Stage::AfterBlock(id);
        let list = reader.peek_is('[').then(|| reader.cursor());
        if list.is_some() {
            block_list(reader, lookup, (id, span), stage)?;
        }
        (BlockOrTag::Block(id), list)
    };
    let data = if reader.peek_is('{') {
        *stage = Stage::Data;
        let start = reader.cursor();
        let end = check_compound(reader.input(), start)
            .map_err(|error| SyntaxError::argument(ArgumentError::Nbt(error), error.position))?;
        reader.set_cursor(end);
        Some(start)
    } else {
        None
    };
    Ok(BlockPredicate {
        subject,
        input: reader.input(),
        list,
        data,
    })
}

impl<L: Lookup> Argument<Source<'_, L>> for BlockPredicateArgument {
    type Value<'a> = BlockPredicate<'a>;
    type Error = ArgumentError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<BlockPredicate<'a>, CommandSyntaxError> {
        read(reader, source.lookup, &mut Stage::Start)
    }
}

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
    entries(input, Some(list)).any(|(name, _)| StringValue::new(input, name).eq_str(&property.name))
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

fn has_block_entity<L: Lookup>(lookup: &L, subject: BlockOrTag<'_>) -> bool {
    match subject {
        BlockOrTag::Block(block) => lookup.has_block_entity(block),
        BlockOrTag::Tag(tag) => lookup
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

impl BlockPredicateArgument {
    /// ARG-BLK-013 and ARG-BLK-014.
    pub fn suggest<'t, L: Lookup, T: Copy>(
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
        let has_block_entity = |subject| has_block_entity(lookup, subject);
        match stage {
            Stage::Start => {
                let remaining = builder.remaining();
                let tags = lookup.tags(registry).map(|tag| (tag, None));
                suggest_prefixed(
                    &mut builder,
                    &["#"],
                    tags,
                    remaining,
                    prefixed_identifier_matches,
                );
                builder.suggest_matching(lookup.entries(registry), identifier_matches);
            }
            Stage::AfterBlock(block) => {
                if lookup
                    .block_properties(block)
                    .is_some_and(|properties| !properties.is_empty())
                {
                    if_empty(&mut builder, "[");
                }
                if has_block_entity(BlockOrTag::Block(block)) {
                    if_empty(&mut builder, "{");
                }
            }
            Stage::AfterTag(tag) => {
                if members(tag).any(|properties| !properties.is_empty()) {
                    if_empty(&mut builder, "[");
                }
                if has_block_entity(BlockOrTag::Tag(tag)) {
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
                        if name.eq_str(&property.name) {
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
                if has_block_entity(subject) {
                    if_empty(&mut builder, "{");
                }
            }
            Stage::Data => {}
        }
    }
}
