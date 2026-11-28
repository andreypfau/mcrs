use core::num::NonZeroU32;

use fixedbitset::FixedBitSet;
use glam::DVec3;
use mcrs_minecraft_item::component::predicate::MobEffectsPredicate;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_voxel::BlockPos;

use crate::baked_location_predicate::BakedLocationPredicate;
use crate::distance_predicate::DistancePredicate;
use crate::entity_flags_predicate::EntityFlagsPredicate;
use crate::entity_tags_predicate::EntityTagsPredicate;
use crate::entity_view::EntityView;
use crate::equipment_predicate::BakedEquipmentTest;
use crate::location_view::LocationView;
use crate::movement_predicate::MovementPredicate;
use crate::predicate_scratch::PredicateScratch;
use crate::predicate_tables::PredicateTables;
use crate::slots_predicate::BakedSlotTest;

#[derive(Clone, Debug, PartialEq)]
pub struct BakedEntityPredicate {
    pub(crate) entity_type: Option<FixedBitSet>,
    pub(crate) location: Option<BakedLocationPredicate>,
    pub(crate) stepping_on: Option<BakedLocationPredicate>,
    pub(crate) movement_affected_by: Option<BakedLocationPredicate>,
    pub(crate) distance: Option<DistancePredicate>,
    pub(crate) movement: Option<MovementPredicate>,
    pub(crate) effects: Option<MobEffectsPredicate>,
    pub(crate) nbt: Option<NbtCompound>,
    pub(crate) flags: Option<EntityFlagsPredicate>,
    pub(crate) living: LivingTest,
    pub(crate) equipment: Box<[BakedEquipmentTest]>,
    pub(crate) periodic_tick: Option<NonZeroU32>,
    pub(crate) vehicle: Option<Box<BakedEntityPredicate>>,
    pub(crate) passenger: Option<Box<BakedEntityPredicate>>,
    pub(crate) targeted_entity: Option<Box<BakedEntityPredicate>>,
    pub(crate) team: Option<String>,
    pub(crate) slots: Box<[BakedSlotTest]>,
    pub(crate) entity_tags: Option<EntityTagsPredicate>,
    pub(crate) deferred: DeferredTest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredTest {
    Absent,
    Unmatched,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LivingTest {
    Any,
    Living,
}

const MAX_NESTED_MATCHES: u32 = 1 << 16;

impl BakedEntityPredicate {
    #[must_use]
    pub fn matches<W: LocationView + EntityView>(
        &self,
        world: &W,
        origin: Option<DVec3>,
        entity: Option<W::Entity>,
        tables: PredicateTables<'_>,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let Some(entity) = entity else {
            return false;
        };
        let context = Context {
            world,
            origin,
            tables,
        };
        start_budget(scratch);
        self.matches_entity(&context, entity, scratch) && within_budget(scratch)
    }

    fn matches_nested<W: LocationView + EntityView>(
        &self,
        context: &Context<'_, W>,
        entity: W::Entity,
        scratch: &mut PredicateScratch,
    ) -> bool {
        if scratch.nested_matches >= MAX_NESTED_MATCHES {
            scratch.nested_matches = MAX_NESTED_MATCHES + 1;
            return false;
        }
        scratch.nested_matches += 1;
        self.matches_entity(context, entity, scratch)
    }

    fn matches_entity<W: LocationView + EntityView>(
        &self,
        context: &Context<'_, W>,
        entity: W::Entity,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let world = context.world;
        self.entity_type.as_ref().is_none_or(|types| {
            world
                .entity_type(entity)
                .is_some_and(|kind| types.contains(kind.index()))
        }) && self.matches_places(context, entity, scratch)
            && self
                .movement
                .is_none_or(|movement| movement.matches(world, entity))
            && self.matches_state(context, entity, scratch)
            && self.matches_relatives(context, entity, scratch)
            && match self.deferred {
                DeferredTest::Absent => true,
                DeferredTest::Unmatched => false,
            }
    }

    fn matches_places<W: LocationView + EntityView>(
        &self,
        context: &Context<'_, W>,
        entity: W::Entity,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let world = context.world;
        let at = |predicate: &BakedLocationPredicate,
                  position: Option<DVec3>,
                  scratch: &mut PredicateScratch| {
            position.is_some_and(|position| {
                predicate.matches(world, position, context.tables.smoke, scratch)
            })
        };
        if let Some(location) = &self.location
            && !at(location, world.position(entity), scratch)
        {
            return false;
        }
        if let Some(stepping_on) = &self.stepping_on {
            let support = match world.is_on_ground(entity) {
                Some(true) => world.support_position(entity),
                Some(false) | None => None,
            };
            if !at(stepping_on, support.map(block_centre), scratch) {
                return false;
            }
        }
        if let Some(affected_by) = &self.movement_affected_by
            && !at(
                affected_by,
                world.movement_position(entity).map(block_centre),
                scratch,
            )
        {
            return false;
        }
        self.distance.is_none_or(|distance| {
            context
                .origin
                .zip(world.position(entity))
                .is_some_and(|(origin, position)| distance.matches(origin, position))
        })
    }

    fn matches_state<W: LocationView + EntityView>(
        &self,
        context: &Context<'_, W>,
        entity: W::Entity,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let world = context.world;
        let living = match self.living {
            LivingTest::Any => true,
            LivingTest::Living => world.is_living(entity) == Some(true),
        };
        let effects = self.effects.as_ref().is_none_or(|effects| {
            world.is_living(entity) == Some(true)
                && effects.0.iter().all(|(kind, expected)| {
                    world.active_effect(entity, *kind).is_some_and(|effect| {
                        expected.amplifier.matches(effect.amplifier)
                            && expected.duration.matches(effect.duration)
                            && expected
                                .ambient
                                .is_none_or(|ambient| ambient == effect.ambient)
                            && expected
                                .visible
                                .is_none_or(|visible| visible == effect.visible)
                    })
                })
        });
        let nbt = self.nbt.as_ref().is_none_or(|nbt| {
            let data = &mut scratch.entity;
            data.child_tags.clear();
            world.saved_data(entity, data) && nbt.matches(data)
        });
        living
            && effects
            && nbt
            && self
                .equipment
                .iter()
                .all(|test| test.matches(world, entity, context.tables.items))
            && self.flags.is_none_or(|flags| flags.matches(world, entity))
            && self.periodic_tick.is_none_or(|period| {
                world
                    .tick_count(entity)
                    .is_some_and(|ticks| ticks % period == 0)
            })
            && self
                .team
                .as_deref()
                .is_none_or(|team| world.team(entity) == Some(team))
            && self
                .slots
                .iter()
                .all(|test| test.matches(world, entity, context.tables.items))
            && self
                .entity_tags
                .as_ref()
                .is_none_or(|tags| tags.matches(world, entity))
    }

    fn matches_relatives<W: LocationView + EntityView>(
        &self,
        context: &Context<'_, W>,
        entity: W::Entity,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let world = context.world;
        self.vehicle.as_deref().is_none_or(|vehicle| {
            world
                .vehicle(entity)
                .is_some_and(|ridden| vehicle.matches_nested(context, ridden, scratch))
        }) && self.passenger.as_deref().is_none_or(|passenger| {
            world.passengers(entity).is_some_and(|mut riders| {
                riders.any(|rider| passenger.matches_nested(context, rider, scratch))
            })
        }) && self.targeted_entity.as_deref().is_none_or(|target| {
            world
                .attack_target(entity)
                .is_some_and(|targeted| target.matches_nested(context, targeted, scratch))
        })
    }
}

struct Context<'a, W> {
    world: &'a W,
    origin: Option<DVec3>,
    tables: PredicateTables<'a>,
}

fn start_budget(scratch: &mut PredicateScratch) {
    scratch.nested_matches = 0;
}

fn within_budget(scratch: &PredicateScratch) -> bool {
    scratch.nested_matches <= MAX_NESTED_MATCHES
}

fn block_centre(pos: BlockPos) -> DVec3 {
    pos.as_ivec3().as_dvec3() + DVec3::splat(0.5)
}
