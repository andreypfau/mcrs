use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_item::component::predicate::BlockPredicate;
use mcrs_minecraft_registries::{Named, Refs, Registries, Resolve, Resolved};
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use serde::{Deserialize, Deserializer};

use crate::baked_block_predicate::BakedBlockPredicate;
use crate::baked_location_predicate::{BakedLocationPredicate, DimensionTest};
use crate::fluid_predicate::FluidPredicate;
use crate::id_bits::{id_bits, members};
use crate::light_predicate::LightPredicate;
use crate::predicate_issue::PredicateIssue;

#[derive(Clone, Debug, PartialEq, Resolve)]
pub struct LocationPredicate<M: Refs = Resolved> {
    position: PositionBounds,
    biomes: Option<M::Set<Biome>>,
    structures: Option<M::Set<Structure>>,
    dimension: Option<ResourceKeyBuf<Dimension>>,
    smokey: Option<bool>,
    light: Option<LightPredicate>,
    block: Option<BlockPredicate<M>>,
    fluid: Option<FluidPredicate<M>>,
    can_see_sky: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub(crate) struct PositionBounds {
    #[serde(default)]
    pub(crate) x: MinMaxBounds<f64>,
    #[serde(default)]
    pub(crate) y: MinMaxBounds<f64>,
    #[serde(default)]
    pub(crate) z: MinMaxBounds<f64>,
}

impl<'de> Deserialize<'de> for LocationPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            #[serde(default)]
            position: PositionBounds,
            #[serde(default)]
            biomes: Option<<Named as Refs>::Set<Biome>>,
            #[serde(default)]
            structures: Option<<Named as Refs>::Set<Structure>>,
            #[serde(default)]
            dimension: Option<ResourceKeyBuf<Dimension>>,
            #[serde(default)]
            smokey: Option<bool>,
            #[serde(default)]
            light: Option<LightPredicate>,
            #[serde(default)]
            block: Option<BlockPredicate<Named>>,
            #[serde(default)]
            fluid: Option<FluidPredicate<Named>>,
            #[serde(default)]
            can_see_sky: Option<bool>,
        }

        let repr = Repr::deserialize(d)?;
        Ok(LocationPredicate {
            position: repr.position,
            biomes: repr.biomes,
            structures: repr.structures,
            dimension: repr.dimension,
            smokey: repr.smokey,
            light: repr.light,
            block: repr.block,
            fluid: repr.fluid,
            can_see_sky: repr.can_see_sky,
        })
    }
}

impl LocationPredicate {
    pub fn bake(
        &self,
        registries: &Registries,
        definitions: &BlockDefinitions,
    ) -> Result<BakedLocationPredicate, PredicateIssue> {
        let biomes = self
            .biomes
            .as_ref()
            .map(|set| id_bits(set, registries, PredicateIssue::UnloadedBiomeTags))
            .transpose()?;
        let structures = self
            .structures
            .as_ref()
            .map(|set| {
                members(set, registries, PredicateIssue::UnloadedStructureTags)
                    .map(Vec::into_boxed_slice)
            })
            .transpose()?;
        let dimension = match &self.dimension {
            None => DimensionTest::Any,
            Some(key) => {
                let dimensions = registries
                    .entries::<Dimension>()
                    .ok_or(PredicateIssue::UnloadedDimensions)?;
                match dimensions.by_name(key.location()) {
                    Some(id) => DimensionTest::Is(id),
                    None => DimensionTest::Unknown,
                }
            }
        };
        let block = self
            .block
            .as_ref()
            .map(|block| BakedBlockPredicate::bake(block, registries, definitions))
            .transpose()?;
        let fluid = self
            .fluid
            .as_ref()
            .map(|fluid| fluid.bake(registries))
            .transpose()?;
        Ok(BakedLocationPredicate {
            position: self.position,
            biomes,
            structures,
            dimension,
            smokey: self.smokey,
            light: self.light,
            block,
            fluid,
            can_see_sky: self.can_see_sky,
        })
    }
}
