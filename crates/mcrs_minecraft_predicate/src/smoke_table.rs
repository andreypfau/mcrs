use fixedbitset::FixedBitSet;
use glam::Vec3;
use mcrs_minecraft_block::block_tag_keys::CAMPFIRES;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_core::voxel_shape::Aabb;
use mcrs_minecraft_registries::Registries;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::location_view::LocationView;
use crate::predicate_issue::PredicateIssue;

const DEPTH: i32 = 5;
const CENTRE_POST: Aabb = Aabb {
    min: Vec3::new(6.0 / 16.0, 0.0, 6.0 / 16.0),
    max: Vec3::new(10.0 / 16.0, 1.0, 10.0 / 16.0),
};

#[derive(Clone, Debug, PartialEq)]
pub struct SmokeTable {
    lit_campfires: FixedBitSet,
    centre_post: FixedBitSet,
}

impl SmokeTable {
    pub fn bake(
        registries: &Registries,
        definitions: &BlockDefinitions,
    ) -> Result<Self, PredicateIssue> {
        let tags = registries
            .tags::<mcrs_minecraft_block::block_keys::Block>()
            .ok_or(PredicateIssue::UnloadedBlockTags)?;
        let campfires = tags.id(CAMPFIRES);
        let lit_campfires = definitions
            .states()
            .filter(|&state| {
                let campfire = campfires
                    .zip(definitions.block_index(state))
                    .is_some_and(|(tag, block)| tags.contains(tag, block));
                campfire
                    && definitions
                        .owner(state)
                        .and_then(|block| block.value_of(state, "lit"))
                        .is_some_and(|lit| *lit == PropertyValue::Bool(true))
            })
            .map(|state| state.index())
            .collect();
        let centre_post = definitions
            .states()
            .filter(|&state| {
                let shape = definitions.shape(definitions.state(state).collision_shape);
                shape.iter().any(intersects_centre_post)
            })
            .map(|state| state.index())
            .collect();
        Ok(SmokeTable {
            lit_campfires,
            centre_post,
        })
    }

    pub(crate) fn smoke<W: LocationView>(&self, world: &W, pos: BlockPos) -> Option<bool> {
        for depth in 1..=DEPTH {
            let state = world.block_state(pos.offset(0, -depth, 0))?;
            if self.lit(state) {
                return Some(true);
            }
            if self.centre_post.contains(state.index()) {
                let under = world.block_state(pos.offset(0, -depth - 1, 0))?;
                return Some(self.lit(under));
            }
        }
        Some(false)
    }

    fn lit(&self, state: BlockStateId) -> bool {
        self.lit_campfires.contains(state.index())
    }
}

fn intersects_centre_post(part: &Aabb) -> bool {
    part.min.cmplt(CENTRE_POST.max).all() && part.max.cmpgt(CENTRE_POST.min).all()
}
