use mcrs_minecraft_value_provider::bounds::MinMaxBounds;

use crate::baked_damage_source_predicate::BakedDamageSourcePredicate;
use crate::baked_entity_predicate::BakedEntityPredicate;
use crate::damage_event::DamageEvent;
use crate::entity_view::EntityView;
use crate::location_view::LocationView;
use crate::predicate_scratch::PredicateScratch;
use crate::predicate_tables::PredicateTables;

#[derive(Clone, Debug, PartialEq)]
pub struct BakedDamagePredicate {
    pub(crate) dealt: MinMaxBounds<f64>,
    pub(crate) taken: MinMaxBounds<f64>,
    pub(crate) source_entity: Option<BakedEntityPredicate>,
    pub(crate) blocked: Option<bool>,
    pub(crate) source: Option<BakedDamageSourcePredicate>,
}

impl BakedDamagePredicate {
    #[must_use]
    pub fn matches<W: LocationView + EntityView>(
        &self,
        world: &W,
        event: &DamageEvent<W::Entity>,
        tables: PredicateTables<'_>,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let origin = world.position(event.player);
        self.dealt.matches(event.dealt)
            && self.taken.matches(event.taken)
            && self.blocked.is_none_or(|blocked| blocked == event.blocked)
            && self.source_entity.as_ref().is_none_or(|predicate| {
                predicate.matches(world, origin, event.source.causing_entity, tables, scratch)
            })
            && self.source.as_ref().is_none_or(|predicate| {
                predicate.matches(world, origin, Some(event.source), tables, scratch)
            })
    }
}
