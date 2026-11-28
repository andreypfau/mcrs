use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_registries::{Named, Refs, Registries, Resolve, Resolved};
use serde::{Deserialize, Deserializer};

use crate::baked_damage_source_predicate::{BakedDamageSourcePredicate, BakedDamageTag};
use crate::entity_predicate::EntityPredicate;
use crate::id_bits::id_bits;
use crate::predicate_issue::PredicateIssue;

#[derive(Clone, Debug, PartialEq, Resolve)]
pub struct DamageSourcePredicate<M: Refs = Resolved> {
    tags: Vec<DamageTagPredicate<M>>,
    direct_entity: Option<EntityPredicate<M>>,
    source_entity: Option<EntityPredicate<M>>,
    is_direct: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) struct DamageTagPredicate<M: Refs = Resolved> {
    id: M::Set<DamageType>,
    expected: bool,
}

impl<'de> Deserialize<'de> for DamageSourcePredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            #[serde(default)]
            tags: Vec<DamageTagPredicate<Named>>,
            #[serde(default)]
            direct_entity: Option<EntityPredicate<Named>>,
            #[serde(default)]
            source_entity: Option<EntityPredicate<Named>>,
            #[serde(default)]
            is_direct: Option<bool>,
        }

        let Repr {
            tags,
            direct_entity,
            source_entity,
            is_direct,
        } = Repr::deserialize(d)?;
        Ok(DamageSourcePredicate {
            tags,
            direct_entity,
            source_entity,
            is_direct,
        })
    }
}

impl<'de> Deserialize<'de> for DamageTagPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            id: <Named as Refs>::Set<DamageType>,
            expected: bool,
        }

        let Repr { id, expected } = Repr::deserialize(d)?;
        Ok(DamageTagPredicate { id, expected })
    }
}

impl DamageSourcePredicate {
    pub fn bake<L: NameLookup>(
        &self,
        registries: &Registries,
        definitions: &BlockDefinitions,
        names: &L,
    ) -> Result<BakedDamageSourcePredicate, PredicateIssue> {
        let tags = self
            .tags
            .iter()
            .map(|tag| {
                Ok(BakedDamageTag {
                    types: id_bits(&tag.id, registries, PredicateIssue::UnloadedDamageTypeTags)?,
                    expected: tag.expected,
                })
            })
            .collect::<Result<_, PredicateIssue>>()?;
        let entity = |predicate: Option<&EntityPredicate>| {
            predicate
                .map(|predicate| predicate.bake(registries, definitions, names))
                .transpose()
        };
        Ok(BakedDamageSourcePredicate {
            tags,
            direct_entity: entity(self.direct_entity.as_ref())?,
            source_entity: entity(self.source_entity.as_ref())?,
            is_direct: self.is_direct,
        })
    }
}
