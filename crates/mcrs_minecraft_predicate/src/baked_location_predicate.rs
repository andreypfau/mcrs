use fixedbitset::FixedBitSet;
use glam::DVec3;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

use crate::baked_block_predicate::BakedBlockPredicate;
use crate::baked_fluid_predicate::BakedFluidPredicate;
use crate::light_predicate::LightPredicate;
use crate::location_predicate::PositionBounds;
use crate::location_view::LocationView;
use crate::predicate_scratch::PredicateScratch;
use crate::smoke_table::SmokeTable;

const FULL_SKY_LIGHT: u8 = 15;

#[derive(Clone, Debug, PartialEq)]
pub struct BakedLocationPredicate {
    pub(crate) position: PositionBounds,
    pub(crate) biomes: Option<FixedBitSet>,
    pub(crate) structures: Option<Box<[Id<Structure>]>>,
    pub(crate) dimension: DimensionTest,
    pub(crate) smokey: Option<bool>,
    pub(crate) light: Option<LightPredicate>,
    pub(crate) block: Option<BakedBlockPredicate>,
    pub(crate) fluid: Option<BakedFluidPredicate>,
    pub(crate) can_see_sky: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DimensionTest {
    Any,
    Is(Id<Dimension>),
    Unknown,
}

impl BakedLocationPredicate {
    #[must_use]
    pub fn matches<W: LocationView>(
        &self,
        world: &W,
        position: DVec3,
        smoke: &SmokeTable,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let dimension = match self.dimension {
            DimensionTest::Any => true,
            DimensionTest::Is(id) => world.dimension() == Some(id),
            DimensionTest::Unknown => false,
        };
        if !(self.position.x.matches(position.x)
            && self.position.y.matches(position.y)
            && self.position.z.matches(position.z)
            && dimension)
        {
            return false;
        }
        if !self.reads_world() {
            return true;
        }
        let pos = BlockPos::from(position);
        if !world.is_loaded(pos) {
            return false;
        }
        self.biomes.as_ref().is_none_or(|biomes| {
            world
                .biome(pos)
                .is_some_and(|biome| biomes.contains(biome.index()))
        }) && self
            .structures
            .as_deref()
            .is_none_or(|structures| inside_any(world, pos, structures))
            && self
                .smokey
                .is_none_or(|expected| smoke.smoke(world, pos) == Some(expected))
            && self.light.is_none_or(|light| light.matches(world, pos))
            && self
                .block
                .as_ref()
                .is_none_or(|block| block.matches(world, pos, scratch))
            && self
                .fluid
                .as_ref()
                .is_none_or(|fluid| fluid.matches(world, pos))
            && self.can_see_sky.is_none_or(|expected| {
                world
                    .sky_light(pos)
                    .is_some_and(|sky| (sky == FULL_SKY_LIGHT) == expected)
            })
    }

    fn reads_world(&self) -> bool {
        self.biomes.is_some()
            || self.structures.is_some()
            || self.smokey.is_some()
            || self.light.is_some()
            || self.block.is_some()
            || self.fluid.is_some()
            || self.can_see_sky.is_some()
    }
}

fn inside_any<W: LocationView>(world: &W, pos: BlockPos, structures: &[Id<Structure>]) -> bool {
    for &structure in structures {
        match world.inside_structure(pos, structure) {
            Some(true) => return true,
            Some(false) => {}
            None => return false,
        }
    }
    false
}
