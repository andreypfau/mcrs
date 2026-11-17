use fixedbitset::FixedBitSet;
use glam::DVec3;

use crate::baked_entity_predicate::BakedEntityPredicate;
use crate::damage_source::DamageSource;
use crate::entity_view::EntityView;
use crate::location_view::LocationView;
use crate::predicate_scratch::PredicateScratch;
use crate::predicate_tables::PredicateTables;

#[derive(Clone, Debug, PartialEq)]
pub struct BakedDamageSourcePredicate {
    pub(crate) tags: Box<[BakedDamageTag]>,
    pub(crate) direct_entity: Option<BakedEntityPredicate>,
    pub(crate) source_entity: Option<BakedEntityPredicate>,
    pub(crate) is_direct: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedDamageTag {
    pub(crate) types: FixedBitSet,
    pub(crate) expected: bool,
}

impl BakedDamageSourcePredicate {
    #[must_use]
    pub fn matches<W: LocationView + EntityView>(
        &self,
        world: &W,
        origin: Option<DVec3>,
        source: Option<DamageSource<W::Entity>>,
        tables: PredicateTables<'_>,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let Some(source) = source else {
            return false;
        };
        let entity = |predicate: Option<&BakedEntityPredicate>,
                      entity: Option<W::Entity>,
                      scratch: &mut PredicateScratch| {
            predicate
                .is_none_or(|predicate| predicate.matches(world, origin, entity, tables, scratch))
        };
        self.tags
            .iter()
            .all(|tag| tag.types.contains(source.damage_type.index()) == tag.expected)
            && self
                .is_direct
                .is_none_or(|direct| direct == source.is_direct())
            && entity(self.direct_entity.as_ref(), source.direct_entity, scratch)
            && entity(self.source_entity.as_ref(), source.causing_entity, scratch)
    }
}
