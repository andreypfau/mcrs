use fixedbitset::FixedBitSet;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry};
use mcrs_minecraft_item::component::predicate::BlockPredicate;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registries::Registries;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::id_bits::members;
use crate::location_view::LocationView;
use crate::predicate_issue::PredicateIssue;
use crate::predicate_scratch::PredicateScratch;
use crate::property_order;

#[derive(Clone, Debug, PartialEq)]
pub struct BakedBlockPredicate {
    states: Option<FixedBitSet>,
    nbt: Option<NbtCompound>,
    components: ComponentTest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComponentTest {
    Absent,
    Unmatched,
}

impl BakedBlockPredicate {
    pub fn bake(
        predicate: &BlockPredicate,
        registries: &Registries,
        definitions: &BlockDefinitions,
    ) -> Result<Self, PredicateIssue> {
        let state = predicate.state.as_ref().filter(|state| !state.is_any());
        let states = match (&predicate.blocks, state) {
            (None, None) => None,
            (blocks, state) => {
                let blocks: Vec<&BlockEntry> = match blocks {
                    Some(set) => members(set, registries, PredicateIssue::UnloadedBlockTags)?
                        .into_iter()
                        .filter_map(|block| definitions.entry(block))
                        .collect(),
                    None => definitions.blocks().iter().collect(),
                };
                let mut bits = FixedBitSet::with_capacity(definitions.state_count());
                for block in blocks {
                    let passing = (0..block.state_count)
                        .filter_map(|offset| block.state_at(offset))
                        .filter(|&id| {
                            state.is_none_or(|state| property_order::passes(state, block, id))
                        });
                    bits.extend(passing.map(|id| id.index()));
                }
                Some(bits)
            }
        };
        let components = match predicate.matchers.is_empty() {
            true => ComponentTest::Absent,
            false => ComponentTest::Unmatched,
        };
        Ok(BakedBlockPredicate {
            states,
            nbt: predicate.nbt.as_ref().map(|nbt| nbt.0.clone()),
            components,
        })
    }

    #[must_use]
    pub(crate) fn matches<W: LocationView>(
        &self,
        world: &W,
        pos: BlockPos,
        scratch: &mut PredicateScratch,
    ) -> bool {
        if !world.is_loaded(pos) {
            return false;
        }
        let data = &mut scratch.block_entity;
        let block_entity = match self.nbt {
            Some(_) => {
                data.child_tags.clear();
                world.block_entity_data(pos, data).then_some(&*data)
            }
            None => None,
        };
        self.matches_state(world.block_state(pos), block_entity)
    }

    #[must_use]
    pub fn matches_state(
        &self,
        state: Option<BlockStateId>,
        block_entity: Option<&NbtCompound>,
    ) -> bool {
        if let Some(states) = &self.states
            && !state.is_some_and(|state| states.contains(state.index()))
        {
            return false;
        }
        if let Some(nbt) = &self.nbt
            && !block_entity.is_some_and(|data| nbt.matches(data))
        {
            return false;
        }
        match self.components {
            ComponentTest::Absent => true,
            ComponentTest::Unmatched => false,
        }
    }
}
