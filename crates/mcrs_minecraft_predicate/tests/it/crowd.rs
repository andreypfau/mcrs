use glam::DVec3;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_predicate::{
    ActiveEffect, EntityView, ItemSubject, LocationView, PredicateTables,
};
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::grid::{Grid, smoke};
use crate::item_predicate::TABLE;

#[derive(Clone, Default)]
pub struct Mob {
    pub entity_type: Option<Id<EntityType>>,
    pub position: Option<DVec3>,
    pub on_ground: Option<bool>,
    pub support: Option<BlockPos>,
    pub movement_position: Option<BlockPos>,
    pub movement: Option<DVec3>,
    pub fall_distance: Option<f64>,
    pub saved_data: Option<NbtCompound>,
    pub burning: Option<bool>,
    pub pose: Option<Pose>,
    pub sprinting: Option<bool>,
    pub swimming: Option<bool>,
    pub in_water: Option<bool>,
    pub living: Option<bool>,
    pub fall_flying: Option<bool>,
    pub flying_ability: Option<bool>,
    pub baby: Option<bool>,
    pub effects: Vec<(Id<MobEffect>, ActiveEffect)>,
    pub tick_count: Option<u32>,
    pub vehicle: Option<usize>,
    pub passengers: Option<Vec<usize>>,
    pub attack_target: Option<usize>,
    pub team: Option<String>,
    pub tags: Option<Vec<String>>,
    pub equipment: Option<Vec<Worn>>,
    pub slots: Option<Vec<Held>>,
}

#[derive(Clone)]
pub struct Worn {
    pub slot: EquipmentSlot,
    pub stack: Option<ItemStack>,
}

#[derive(Clone)]
pub struct Held {
    pub slot: u16,
    pub stack: Option<ItemStack>,
}

pub fn tables() -> PredicateTables<'static> {
    PredicateTables::new(smoke(), &TABLE)
}

fn subject(stack: Option<&ItemStack>) -> ItemSubject<'_> {
    match stack {
        Some(stack) => ItemSubject::Stack(stack),
        None => ItemSubject::Empty,
    }
}

#[derive(Default)]
pub struct Crowd {
    pub grid: Grid,
    pub mobs: Vec<Mob>,
}

impl Crowd {
    fn mob(&self, entity: usize) -> &Mob {
        self.mobs.get(entity).expect("an entity of the crowd")
    }
}

impl EntityView for Crowd {
    type Entity = usize;

    fn entity_type(&self, entity: usize) -> Option<Id<EntityType>> {
        self.mob(entity).entity_type
    }

    fn position(&self, entity: usize) -> Option<DVec3> {
        self.mob(entity).position
    }

    fn is_on_ground(&self, entity: usize) -> Option<bool> {
        self.mob(entity).on_ground
    }

    fn support_position(&self, entity: usize) -> Option<BlockPos> {
        self.mob(entity).support
    }

    fn movement_position(&self, entity: usize) -> Option<BlockPos> {
        self.mob(entity).movement_position
    }

    fn reported_movement(&self, entity: usize) -> Option<DVec3> {
        self.mob(entity).movement
    }

    fn fall_distance(&self, entity: usize) -> Option<f64> {
        self.mob(entity).fall_distance
    }

    fn saved_data(&self, entity: usize, data: &mut NbtCompound) -> bool {
        match &self.mob(entity).saved_data {
            Some(saved) => {
                data.merge(saved);
                true
            }
            None => false,
        }
    }

    fn is_burning(&self, entity: usize) -> Option<bool> {
        self.mob(entity).burning
    }

    fn pose(&self, entity: usize) -> Option<Pose> {
        self.mob(entity).pose
    }

    fn is_sprinting(&self, entity: usize) -> Option<bool> {
        self.mob(entity).sprinting
    }

    fn is_swimming(&self, entity: usize) -> Option<bool> {
        self.mob(entity).swimming
    }

    fn is_in_water(&self, entity: usize) -> Option<bool> {
        self.mob(entity).in_water
    }

    fn is_living(&self, entity: usize) -> Option<bool> {
        self.mob(entity).living
    }

    fn is_fall_flying(&self, entity: usize) -> Option<bool> {
        self.mob(entity).fall_flying
    }

    fn has_flying_ability(&self, entity: usize) -> Option<bool> {
        self.mob(entity).flying_ability
    }

    fn is_baby(&self, entity: usize) -> Option<bool> {
        self.mob(entity).baby
    }

    fn active_effect(&self, entity: usize, effect: Id<MobEffect>) -> Option<ActiveEffect> {
        self.mob(entity)
            .effects
            .iter()
            .find(|(kind, _)| *kind == effect)
            .map(|&(_, active)| active)
    }

    fn equipment(&self, entity: usize, slot: EquipmentSlot) -> Option<ItemSubject<'_>> {
        let worn = self.mob(entity).equipment.as_deref()?;
        let stack = worn.iter().find(|worn| worn.slot == slot);
        Some(subject(stack.and_then(|worn| worn.stack.as_ref())))
    }

    fn slot(&self, entity: usize, slot: u16) -> Option<ItemSubject<'_>> {
        let held = self.mob(entity).slots.as_deref()?;
        let held = held.iter().find(|held| held.slot == slot)?;
        Some(subject(held.stack.as_ref()))
    }

    fn tick_count(&self, entity: usize) -> Option<u32> {
        self.mob(entity).tick_count
    }

    fn vehicle(&self, entity: usize) -> Option<usize> {
        self.mob(entity).vehicle
    }

    fn passengers(&self, entity: usize) -> Option<impl Iterator<Item = usize>> {
        self.mob(entity)
            .passengers
            .as_deref()
            .map(|riders| riders.iter().copied())
    }

    fn attack_target(&self, entity: usize) -> Option<usize> {
        self.mob(entity).attack_target
    }

    fn team(&self, entity: usize) -> Option<&str> {
        self.mob(entity).team.as_deref()
    }

    fn has_scoreboard_tag(&self, entity: usize, tag: &str) -> Option<bool> {
        self.mob(entity)
            .tags
            .as_deref()
            .map(|tags| tags.iter().any(|held| held == tag))
    }
}

impl LocationView for Crowd {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId> {
        self.grid.block_state(pos)
    }

    fn is_loaded(&self, pos: BlockPos) -> bool {
        self.grid.is_loaded(pos)
    }

    fn fluid_state(&self, pos: BlockPos) -> Option<FluidState> {
        self.grid.fluid_state(pos)
    }

    fn block_light(&self, pos: BlockPos) -> Option<u8> {
        self.grid.block_light(pos)
    }

    fn sky_light(&self, pos: BlockPos) -> Option<u8> {
        self.grid.sky_light(pos)
    }

    fn sky_light_level(&self) -> Option<f32> {
        self.grid.sky_light_level()
    }

    fn biome(&self, pos: BlockPos) -> Option<Id<Biome>> {
        self.grid.biome(pos)
    }

    fn inside_structure(&self, pos: BlockPos, structure: Id<Structure>) -> Option<bool> {
        self.grid.inside_structure(pos, structure)
    }

    fn dimension(&self) -> Option<Id<Dimension>> {
        self.grid.dimension()
    }

    fn block_entity_data(&self, pos: BlockPos, data: &mut NbtCompound) -> bool {
        self.grid.block_entity_data(pos, data)
    }
}
