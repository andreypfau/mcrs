use crate::block_lookup::BlockLookup;
use crate::block_predicate::{BlockPredicate, BlockPredicateArgument, Stage, read};
use crate::error::BlockError;
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_command::Source;
use mcrs_minecraft_command_argument::ResourceOrTag;
use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockStateArgument;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockStateInput<'a>(BlockPredicate<'a>);

impl BlockStateInput<'_> {
    #[must_use]
    pub fn state(&self, blocks: &BlockDefinitions) -> Option<BlockStateId> {
        let ResourceOrTag::Entry(block) = self.0.subject else {
            return None;
        };
        let block = block.to_string();
        let entry = blocks.block(&block)?;
        self.0
            .properties()
            .try_fold(entry.default_state_id, |state, (name, value)| {
                entry.with_text(state, name.as_unescaped()?, value.as_unescaped()?)
            })
    }
}

impl BlockStateArgument {
    pub fn suggest<'t, L: BlockLookup, T: Copy>(
        source: &'t Source<'_, L>,
        builder: &mut SuggestionBuilder<'t, '_, T>,
    ) {
        if !builder.remaining().starts_with('#') {
            BlockPredicateArgument::suggest(source, builder);
        }
    }
}

impl<L: BlockLookup> Argument<Source<'_, L>> for BlockStateArgument {
    type Value<'a> = BlockStateInput<'a>;
    type Error = BlockError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &Source<'_, L>,
    ) -> Result<BlockStateInput<'a>, SyntaxError<BlockError>> {
        if reader.peek_is('#') {
            return Err(SyntaxError::argument(
                BlockError::TagDisallowed,
                reader.cursor(),
            ));
        }
        read(reader, source.lookup, &mut Stage::Start).map(BlockStateInput)
    }
}
