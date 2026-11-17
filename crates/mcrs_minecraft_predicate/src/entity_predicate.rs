use core::fmt;
use core::num::NonZeroU32;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_item::ComponentMap;
use mcrs_minecraft_item::component::common::NbtPredicate;
use mcrs_minecraft_item::component::predicate::{ComponentPredicates, MobEffectsPredicate};
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_registries::{Named, Refs, Registries, Resolve, Resolved};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::baked_entity_predicate::{BakedEntityPredicate, DeferredTest, LivingTest};
use crate::distance_predicate::DistancePredicate;
use crate::entity_flags_predicate::EntityFlagsPredicate;
use crate::entity_sub_predicate_type_keys::EntitySubPredicateType;
use crate::entity_tags_predicate::EntityTagsPredicate;
use crate::equipment_predicate::EquipmentPredicate;
use crate::id_bits::id_bits;
use crate::location_predicate::LocationPredicate;
use crate::movement_predicate::MovementPredicate;
use crate::predicate_issue::PredicateIssue;
use crate::slots_predicate::SlotsPredicate;
use crate::type_specific_predicate::{TypeSpecificPredicate, TypeSpecificScope};

pub(crate) const MAX_NESTED_ENTITY_PREDICATES: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Resolve)]
pub struct EntityPredicate<M: Refs = Resolved> {
    entity_type: Option<M::Set<EntityType>>,
    location: Option<LocationPredicate<M>>,
    stepping_on: Option<LocationPredicate<M>>,
    movement_affected_by: Option<LocationPredicate<M>>,
    distance: Option<DistancePredicate>,
    movement: Option<MovementPredicate>,
    effects: Option<MobEffectsPredicate<M>>,
    nbt: Option<NbtPredicate>,
    flags: Option<EntityFlagsPredicate>,
    equipment: Option<EquipmentPredicate<M>>,
    periodic_tick: Option<NonZeroU32>,
    vehicle: Option<Box<EntityPredicate<M>>>,
    passenger: Option<Box<EntityPredicate<M>>>,
    targeted_entity: Option<Box<EntityPredicate<M>>>,
    team: Option<String>,
    slots: Option<SlotsPredicate<M>>,
    components: Option<ComponentMap<M>>,
    predicates: Option<ComponentPredicates<M::Nbt>>,
    entity_tags: Option<EntityTagsPredicate>,
    type_specific: Vec<TypeSpecificPredicate<M>>,
}

impl<'de> Deserialize<'de> for EntityPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(EntityPredicateVisitor)
    }
}

struct EntityPredicateVisitor;

impl<'de> Visitor<'de> for EntityPredicateVisitor {
    type Value = EntityPredicate<Named>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a map of entity sub-predicate types to sub-predicates")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut predicate = EntityPredicate::<Named>::default();
        while let Some(key) = map.next_key::<String>()? {
            let Some(kind) = EntitySubPredicateType::read(&key) else {
                return Err(A::Error::custom(format_args!(
                    "unknown entity sub-predicate type {key}"
                )));
            };
            let repeated = match kind {
                EntitySubPredicateType::EntityType => put(&mut predicate.entity_type, &mut map)?,
                EntitySubPredicateType::Location => put(&mut predicate.location, &mut map)?,
                EntitySubPredicateType::SteppingOn => put(&mut predicate.stepping_on, &mut map)?,
                EntitySubPredicateType::MovementAffectedBy => {
                    put(&mut predicate.movement_affected_by, &mut map)?
                }
                EntitySubPredicateType::Distance => put(&mut predicate.distance, &mut map)?,
                EntitySubPredicateType::Movement => put(&mut predicate.movement, &mut map)?,
                EntitySubPredicateType::Effects => put(&mut predicate.effects, &mut map)?,
                EntitySubPredicateType::Nbt => put(&mut predicate.nbt, &mut map)?,
                EntitySubPredicateType::Flags => put(&mut predicate.flags, &mut map)?,
                EntitySubPredicateType::Equipment => put(&mut predicate.equipment, &mut map)?,
                EntitySubPredicateType::PeriodicTick => {
                    let ticks: i32 = map.next_value()?;
                    let ticks = u32::try_from(ticks).ok().and_then(NonZeroU32::new);
                    let Some(ticks) = ticks else {
                        return Err(A::Error::custom("periodic_tick must be positive"));
                    };
                    predicate.periodic_tick.replace(ticks).is_some()
                }
                EntitySubPredicateType::Vehicle => put(&mut predicate.vehicle, &mut map)?,
                EntitySubPredicateType::Passenger => put(&mut predicate.passenger, &mut map)?,
                EntitySubPredicateType::TargetedEntity => {
                    put(&mut predicate.targeted_entity, &mut map)?
                }
                EntitySubPredicateType::Team => put(&mut predicate.team, &mut map)?,
                EntitySubPredicateType::Slots => put(&mut predicate.slots, &mut map)?,
                EntitySubPredicateType::Components => put(&mut predicate.components, &mut map)?,
                EntitySubPredicateType::Predicates => put(&mut predicate.predicates, &mut map)?,
                EntitySubPredicateType::EntityTags => put(&mut predicate.entity_tags, &mut map)?,
                EntitySubPredicateType::TypeSpecificLightning => push(
                    &mut predicate.type_specific,
                    TypeSpecificPredicate::Lightning(map.next_value()?),
                ),
                EntitySubPredicateType::TypeSpecificFishingHook => push(
                    &mut predicate.type_specific,
                    TypeSpecificPredicate::FishingHook(map.next_value()?),
                ),
                EntitySubPredicateType::TypeSpecificPlayer => push(
                    &mut predicate.type_specific,
                    TypeSpecificPredicate::Player(map.next_value()?),
                ),
                EntitySubPredicateType::TypeSpecificCubeMob => push(
                    &mut predicate.type_specific,
                    TypeSpecificPredicate::CubeMob(map.next_value()?),
                ),
                EntitySubPredicateType::TypeSpecificRaider => push(
                    &mut predicate.type_specific,
                    TypeSpecificPredicate::Raider(map.next_value()?),
                ),
                EntitySubPredicateType::TypeSpecificSheep => push(
                    &mut predicate.type_specific,
                    TypeSpecificPredicate::Sheep(map.next_value()?),
                ),
            };
            if repeated {
                return Err(A::Error::custom(format_args!("Duplicate key '{key}'")));
            }
        }
        Ok(predicate)
    }
}

fn put<'de, T: Deserialize<'de>, A: MapAccess<'de>>(
    slot: &mut Option<T>,
    map: &mut A,
) -> Result<bool, A::Error> {
    let value = map.next_value()?;
    Ok(slot.replace(value).is_some())
}

fn push(
    kinds: &mut Vec<TypeSpecificPredicate<Named>>,
    predicate: TypeSpecificPredicate<Named>,
) -> bool {
    let repeated = kinds
        .iter()
        .any(|seen| core::mem::discriminant(seen) == core::mem::discriminant(&predicate));
    kinds.push(predicate);
    repeated
}

impl EntityPredicate {
    pub fn bake<L: NameLookup>(
        &self,
        registries: &Registries,
        definitions: &BlockDefinitions,
        names: &L,
    ) -> Result<BakedEntityPredicate, PredicateIssue> {
        if !self.nesting_fits(1) {
            return Err(PredicateIssue::NestingTooDeep);
        }
        self.bake_nested(registries, definitions, names)
    }

    fn nesting_fits(&self, depth: usize) -> bool {
        depth <= MAX_NESTED_ENTITY_PREDICATES
            && self.nested().all(|nested| nested.nesting_fits(depth + 1))
    }

    fn nested(&self) -> impl Iterator<Item = &EntityPredicate> {
        let relatives = [
            self.vehicle.as_deref(),
            self.passenger.as_deref(),
            self.targeted_entity.as_deref(),
        ];
        let type_specific = self
            .type_specific
            .iter()
            .filter_map(TypeSpecificPredicate::nested);
        relatives.into_iter().flatten().chain(type_specific)
    }

    fn bake_nested<L: NameLookup>(
        &self,
        registries: &Registries,
        definitions: &BlockDefinitions,
        names: &L,
    ) -> Result<BakedEntityPredicate, PredicateIssue> {
        let nested = |predicate: Option<&EntityPredicate>| {
            predicate
                .map(|predicate| {
                    predicate
                        .bake_nested(registries, definitions, names)
                        .map(Box::new)
                })
                .transpose()
        };
        let location = |predicate: Option<&LocationPredicate>| {
            predicate
                .map(|predicate| predicate.bake(registries, definitions))
                .transpose()
        };
        let mut entity_type = self
            .entity_type
            .as_ref()
            .map(|set| id_bits(set, registries, PredicateIssue::UnloadedEntityTypeTags))
            .transpose()?;
        let mut deferred = match self
            .components
            .as_ref()
            .is_some_and(|map| !map.0.is_empty())
            || self
                .predicates
                .as_ref()
                .is_some_and(|map| !map.0.is_empty())
        {
            true => DeferredTest::Unmatched,
            false => DeferredTest::Absent,
        };
        for predicate in &self.type_specific {
            match predicate.scope() {
                TypeSpecificScope::Every => {}
                TypeSpecificScope::Unmatched => deferred = DeferredTest::Unmatched,
                TypeSpecificScope::EntityTypes(types) => {
                    let required = type_bits(types, registries)?;
                    entity_type = Some(match entity_type {
                        Some(mut allowed) => {
                            allowed.intersect_with(&required);
                            allowed
                        }
                        None => required,
                    });
                }
            }
        }
        let living = match self.equipment {
            Some(_) => LivingTest::Living,
            None => LivingTest::Any,
        };
        let equipment = match &self.equipment {
            Some(equipment) => equipment.bake(registries, names)?,
            None => Box::default(),
        };
        let slots = match &self.slots {
            Some(slots) => slots.bake(registries, names)?,
            None => Box::default(),
        };
        Ok(BakedEntityPredicate {
            entity_type,
            location: location(self.location.as_ref())?,
            stepping_on: location(self.stepping_on.as_ref())?,
            movement_affected_by: location(self.movement_affected_by.as_ref())?,
            distance: self.distance,
            movement: self.movement,
            effects: self.effects.clone(),
            nbt: self.nbt.as_ref().map(|nbt| nbt.0.clone()),
            flags: self.flags,
            living,
            equipment,
            periodic_tick: self.periodic_tick,
            vehicle: nested(self.vehicle.as_deref())?,
            passenger: nested(self.passenger.as_deref())?,
            targeted_entity: nested(self.targeted_entity.as_deref())?,
            team: self.team.clone(),
            slots,
            entity_tags: self.entity_tags.clone(),
            deferred,
        })
    }
}

fn type_bits(types: &[EntityType], registries: &Registries) -> Result<FixedBitSet, PredicateIssue> {
    let entries = registries
        .entries::<EntityType>()
        .ok_or(PredicateIssue::UnloadedEntityTypes)?;
    Ok(types
        .iter()
        .filter_map(|kind| entries.by_name(kind.location()))
        .map(|id| id.index())
        .collect())
}
