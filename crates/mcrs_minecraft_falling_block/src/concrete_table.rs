use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::concrete_pairs::PackPairs;
use crate::concrete_pairs_error::ConcretePairsError;
use crate::concrete_pairs_error::ConcretePairsErrorKind;

/// The concrete of each powder block after the merge of the packs, indexed by block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcreteTable {
    concrete: Box<[Option<Id<Block>>]>,
}

impl ConcreteTable {
    /// Merges the pairs of `packs` in their load order and resolves them against `definitions`
    /// (FBK-TRG-002). A pack with `replace` discards the pairs before it; a later pair of a key
    /// replaces the earlier one. Every pair of every pack must name two different blocks.
    pub fn load<'a>(
        packs: impl IntoIterator<Item = PackPairs<'a>>,
        definitions: &BlockDefinitions,
    ) -> Result<Self, ConcretePairsError> {
        let mut concrete: Box<[Option<Id<Block>>]> =
            vec![None; definitions.blocks().len()].into_boxed_slice();
        for pack in packs {
            if pack.file.replace {
                concrete.fill(None);
            }
            for (powder, block) in &pack.file.values {
                let error = |identifier: &ResourceLocationBuf, kind| ConcretePairsError {
                    pack: pack.pack.to_owned(),
                    identifier: identifier.clone(),
                    kind,
                };
                let powder_id = definitions
                    .id_of(powder.as_str())
                    .ok_or_else(|| error(powder, ConcretePairsErrorKind::UnknownBlock))?;
                let block_id = definitions
                    .id_of(block.as_str())
                    .ok_or_else(|| error(block, ConcretePairsErrorKind::UnknownBlock))?;
                if powder_id == block_id {
                    return Err(error(powder, ConcretePairsErrorKind::MapsToItself));
                }
                if let Some(slot) = concrete.get_mut(powder_id.index()) {
                    *slot = Some(block_id);
                }
            }
        }
        Ok(Self { concrete })
    }

    /// The concrete of `powder`, or `None` when `powder` is not a concrete powder.
    #[must_use]
    pub fn concrete(&self, powder: Id<Block>) -> Option<Id<Block>> {
        self.concrete.get(powder.index()).copied().flatten()
    }
}
