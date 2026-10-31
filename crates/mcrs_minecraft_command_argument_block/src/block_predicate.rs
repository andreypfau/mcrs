use crate::block_lookup::BlockLookup;
use crate::error::{BlockError, BlockSubject};
use mcrs_command::{Argument, Reader, StringSpan, StringValue, SyntaxError};
use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::definition::schema::{BlockProperty, PropertyValue};
use mcrs_minecraft_block::keys::BLOCK;
use mcrs_minecraft_command::{Source, SpannedIdentifier, read_identifier};
use mcrs_minecraft_command_argument::ResourceOrTag;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_snbt::{check_compound, read_compound};
use mcrs_voxel::BlockStateId;

/// The value of a `block_predicate`. The property list and the data compound stay text and are
/// read again by the test, so the value borrows the command line and owns nothing.
// chisle: the test re-reads the property names and the SNBT per call. Resolving property and
// value indices, or holding the parsed `NbtCompound`, needs an owned value, which the
// allocation-free parse test forbids; a caller-owned compiled predicate lifts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockPredicate<'a> {
    pub subject: ResourceOrTag<'a>,
    input: &'a str,
    /// The position of the `[` of the property list.
    list: Option<usize>,
    /// The position of the `{` of the data compound.
    data: Option<usize>,
}

impl<'a> BlockPredicate<'a> {
    fn properties(&self) -> impl Iterator<Item = (StringValue<'a>, StringValue<'a>)> + 'a {
        let input = self.input;
        entries(input, self.list).map(move |(name, value)| {
            (
                StringValue::new(input, name),
                StringValue::new(input, value),
            )
        })
    }

    /// Without a data compound the test passes. With one, `saved` gives the saved
    /// data of the block entity at the position, or `None` when there is none. The
    /// compound is built from the text the read validated.
    #[must_use]
    pub fn test_data(&self, saved: impl FnOnce() -> Option<NbtCompound>) -> bool {
        let Some(start) = self.data else {
            return true;
        };
        let Ok((expected, _)) = read_compound(self.input, start) else {
            return false;
        };
        saved().is_some_and(|actual| expected.matches(&actual))
    }

    /// The property test of a block state. The caller tests that the block of the
    /// state is the block of the predicate, or a member of its tag.
    #[must_use]
    pub fn test(&self, block: &BlockEntry, state: BlockStateId) -> bool {
        self.properties().all(|(name, text)| {
            block
                .properties
                .0
                .iter()
                .find(|property| name == *property.name)
                .and_then(|property| block.value_of(state, &property.name))
                .is_some_and(|value| value_text_is(value, text))
        })
    }
}

/// Reads an optional sign and ASCII digits as a 32-bit integer. A text with an
/// escape holds a backslash, so it is no integer.
fn integer(text: StringValue<'_>) -> Option<i32> {
    text.as_unescaped()?.parse().ok()
}

/// Whether the value text names `value`.
fn value_text_is(value: &PropertyValue, text: StringValue<'_>) -> bool {
    match value {
        PropertyValue::Str(name) => text == **name,
        PropertyValue::Bool(true) => text == "true",
        PropertyValue::Bool(false) => text == "false",
        PropertyValue::Int(number) => integer(text) == Some(*number),
    }
}

/// The pairs of an already validated property list.
pub(crate) fn entries(
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
        let name = reader.read_string::<BlockError>().ok()?;
        reader.skip_whitespace();
        if !reader.peek_is('=') {
            return None;
        }
        reader.skip();
        reader.skip_whitespace();
        let value = reader.read_string::<BlockError>().ok()?;
        reader.skip_whitespace();
        if reader.peek_is(',') {
            reader.skip();
        }
        Some((name, value))
    })
}

/// Where a read stopped, for the suggestions.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Stage<'a> {
    #[default]
    Start,
    AfterSubject(ResourceOrTag<'a>),
    BlockName {
        block: ResourceLocation<'a>,
        list: usize,
        first: bool,
    },
    BlockEquals,
    BlockValue {
        block: ResourceLocation<'a>,
        property: usize,
    },
    BlockAfterValue {
        block: ResourceLocation<'a>,
        list: usize,
    },
    TagName {
        tag: ResourceLocation<'a>,
        list: usize,
        first: bool,
    },
    TagValue {
        tag: ResourceLocation<'a>,
        list: usize,
        name: StringSpan,
    },
    AfterList(ResourceOrTag<'a>),
    Data,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockPredicateArgument;

/// One entry of the property list of a block.
fn block_entry<'a>(
    reader: &mut Reader<'a>,
    properties: &[BlockProperty],
    block: SpannedIdentifier<'a>,
    named: &mut u64,
    stage: &mut Stage<'a>,
) -> Result<(), SyntaxError<BlockError>> {
    let SpannedIdentifier {
        identifier: id,
        span: block_span,
    } = block;
    reader.skip_whitespace();
    let name_start = reader.cursor();
    let name = reader.read_string()?;
    let name_text = StringValue::new(reader.input(), name);
    let Some(index) = properties
        .iter()
        .position(|property| name_text == *property.name)
    else {
        reader.set_cursor(name_start);
        return Err(SyntaxError::argument(
            BlockError::PropertyUnknown {
                block: block_span,
                name,
            },
            name_start,
        ));
    };
    // chisle: one bit per property finds duplicates only among the first 64 properties of a
    // block; vanilla has at most 7. A block with more needs a wider set.
    let bit = 1u64
        .checked_shl(u32::try_from(index).unwrap_or(u32::MAX))
        .unwrap_or(0);
    if *named & bit != 0 {
        reader.set_cursor(name_start);
        return Err(SyntaxError::argument(
            BlockError::PropertyDuplicate {
                name,
                subject: BlockSubject::Block(block_span),
            },
            name_start,
        ));
    }
    *named |= bit;
    reader.skip_whitespace();
    *stage = Stage::BlockEquals;
    if reader.at_end() || !reader.peek_is('=') {
        return Err(SyntaxError::argument(
            BlockError::PropertyNoValue {
                name,
                subject: BlockSubject::Block(block_span),
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
            BlockError::PropertyInvalid {
                block: block_span,
                value,
                name,
            },
            value_start,
        ));
    }
    Ok(())
}

fn block_list<'a, L: BlockLookup>(
    reader: &mut Reader<'a>,
    lookup: &L,
    block: SpannedIdentifier<'a>,
    stage: &mut Stage<'a>,
) -> Result<(), SyntaxError<BlockError>> {
    let id = block.identifier;
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
        block_entry(reader, properties, block, &mut named, stage)?;
        *stage = Stage::BlockAfterValue { block: id, list };
        reader.skip_whitespace();
        if reader.at_end() || reader.peek_is(']') {
            break;
        }
        if !reader.peek_is(',') {
            return Err(SyntaxError::argument(
                BlockError::PropertyUnclosed,
                reader.cursor(),
            ));
        }
        reader.skip();
        first = false;
    }
    if reader.at_end() {
        return Err(SyntaxError::argument(
            BlockError::PropertyUnclosed,
            reader.cursor(),
        ));
    }
    reader.skip();
    *stage = Stage::AfterList(ResourceOrTag::Entry(id));
    Ok(())
}

fn tag_list<'a>(
    reader: &mut Reader<'a>,
    tag: SpannedIdentifier<'a>,
    stage: &mut Stage<'a>,
) -> Result<(), SyntaxError<BlockError>> {
    let id = tag.identifier;
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
        let subject = BlockSubject::Tag(tag.span);
        if duplicate {
            reader.set_cursor(name_start);
            return Err(SyntaxError::argument(
                BlockError::PropertyDuplicate { name, subject },
                name_start,
            ));
        }
        reader.skip_whitespace();
        if reader.at_end() || !reader.peek_is('=') {
            reader.set_cursor(name_start);
            return Err(SyntaxError::argument(
                BlockError::PropertyNoValue { name, subject },
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
                BlockError::PropertyUnclosed,
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
            BlockError::PropertyUnclosed,
            reader.cursor(),
        ));
    }
    reader.skip();
    *stage = Stage::AfterList(ResourceOrTag::Tag(id));
    Ok(())
}

pub(crate) fn read<'a, L: BlockLookup>(
    reader: &mut Reader<'a>,
    lookup: &L,
    stage: &mut Stage<'a>,
) -> Result<BlockPredicate<'a>, SyntaxError<BlockError>> {
    let start = reader.cursor();
    let registry = BLOCK.location();
    let (subject, list) = if reader.peek_is('#') {
        reader.skip();
        let tag = read_identifier(reader)?;
        let id = tag.identifier;
        if !lookup.has_tag(registry, id) {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(
                BlockError::BlockTagUnknown(tag.span),
                start,
            ));
        }
        *stage = Stage::AfterSubject(ResourceOrTag::Tag(id));
        let list = reader.peek_is('[').then(|| reader.cursor());
        if list.is_some() {
            tag_list(reader, tag, stage)?;
        }
        (ResourceOrTag::Tag(id), list)
    } else {
        let block = read_identifier(reader)?;
        let id = block.identifier;
        if !lookup.has_entry(registry, id) {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(
                BlockError::BlockIdInvalid(block.span),
                start,
            ));
        }
        *stage = Stage::AfterSubject(ResourceOrTag::Entry(id));
        let list = reader.peek_is('[').then(|| reader.cursor());
        if list.is_some() {
            block_list(reader, lookup, block, stage)?;
        }
        (ResourceOrTag::Entry(id), list)
    };
    let data = if reader.peek_is('{') {
        *stage = Stage::Data;
        let start = reader.cursor();
        let end = check_compound(reader.input(), start)
            .map_err(|error| SyntaxError::argument(BlockError::Nbt(error), error.position))?;
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

impl<L: BlockLookup> Argument<Source<'_, L>> for BlockPredicateArgument {
    type Value<'a> = BlockPredicate<'a>;
    type Error = BlockError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<BlockPredicate<'a>, SyntaxError<BlockError>> {
        read(reader, source.lookup, &mut Stage::Start)
    }
}
