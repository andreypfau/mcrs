use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_registries::{Named, Refs, Registries, Resolve, Resolved};
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use serde::{Deserialize, Deserializer};

use crate::baked_damage_predicate::BakedDamagePredicate;
use crate::damage_source_predicate::DamageSourcePredicate;
use crate::entity_predicate::EntityPredicate;
use crate::predicate_issue::PredicateIssue;

#[derive(Clone, Debug, PartialEq, Resolve)]
pub struct DamagePredicate<M: Refs = Resolved> {
    dealt: MinMaxBounds<f64>,
    taken: MinMaxBounds<f64>,
    source_entity: Option<EntityPredicate<M>>,
    blocked: Option<bool>,
    source: Option<DamageSourcePredicate<M>>,
}

impl<'de> Deserialize<'de> for DamagePredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            #[serde(default)]
            dealt: MinMaxBounds<f64>,
            #[serde(default)]
            taken: MinMaxBounds<f64>,
            #[serde(default)]
            source_entity: Option<EntityPredicate<Named>>,
            #[serde(default)]
            blocked: Option<bool>,
            #[serde(default, rename = "type")]
            source: Option<DamageSourcePredicate<Named>>,
        }

        let Repr {
            dealt,
            taken,
            source_entity,
            blocked,
            source,
        } = Repr::deserialize(d)?;
        Ok(DamagePredicate {
            dealt,
            taken,
            source_entity,
            blocked,
            source,
        })
    }
}

impl DamagePredicate {
    pub fn bake<L: NameLookup>(
        &self,
        registries: &Registries,
        definitions: &BlockDefinitions,
        names: &L,
    ) -> Result<BakedDamagePredicate, PredicateIssue> {
        Ok(BakedDamagePredicate {
            dealt: self.dealt,
            taken: self.taken,
            source_entity: self
                .source_entity
                .as_ref()
                .map(|predicate| predicate.bake(registries, definitions, names))
                .transpose()?,
            blocked: self.blocked,
            source: self
                .source
                .as_ref()
                .map(|predicate| predicate.bake(registries, definitions, names))
                .transpose()?,
        })
    }
}
