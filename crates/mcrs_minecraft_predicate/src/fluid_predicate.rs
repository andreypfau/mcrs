use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_block_predicate::state_properties::StatePropertiesPredicate;
use mcrs_minecraft_registries::{Named, Refs, Registries, Resolve, Resolved};
use serde::{Deserialize, Deserializer};

use crate::baked_fluid_predicate::{BakedFluidPredicate, FluidStates};
use crate::id_bits::id_bits;
use crate::predicate_issue::PredicateIssue;

#[derive(Clone, Debug, PartialEq, Resolve)]
pub struct FluidPredicate<M: Refs = Resolved> {
    fluids: Option<M::Set<Fluid>>,
    state: Option<StatePropertiesPredicate>,
}

impl<'de> Deserialize<'de> for FluidPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            #[serde(default)]
            fluids: Option<<Named as Refs>::Set<Fluid>>,
            #[serde(default)]
            state: Option<StatePropertiesPredicate>,
        }

        let Repr { fluids, state } = Repr::deserialize(d)?;
        Ok(FluidPredicate { fluids, state })
    }
}

impl FluidPredicate {
    pub(crate) fn bake(
        &self,
        registries: &Registries,
    ) -> Result<BakedFluidPredicate, PredicateIssue> {
        let fluids = self
            .fluids
            .as_ref()
            .map(|set| id_bits(set, registries, PredicateIssue::UnloadedFluidTags))
            .transpose()?;
        let states = self
            .state
            .as_ref()
            .filter(|state| !state.is_any())
            .map(|state| {
                FluidStates::from_test(|level, source| {
                    state.matches_fluid(Some(&FluidState {
                        fluid: Fluid::Water.id(),
                        level,
                        source,
                    }))
                })
            });
        Ok(BakedFluidPredicate { fluids, states })
    }
}
