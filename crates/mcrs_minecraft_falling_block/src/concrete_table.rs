use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::concrete_pairs_error::ConcretePairsError;
use crate::concrete_pairs_error::ConcretePairsErrorKind;
use crate::pack_concrete_pairs::PackConcretePairs;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcreteTable {
    concrete: Box<[Option<Id<Block>>]>,
}

impl ConcreteTable {
    pub fn load(
        packs: &[PackConcretePairs],
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
                    pack: pack.pack.clone(),
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

    #[must_use]
    pub fn concrete(&self, powder: Id<Block>) -> Option<Id<Block>> {
        self.concrete.get(powder.index()).copied().flatten()
    }
}
