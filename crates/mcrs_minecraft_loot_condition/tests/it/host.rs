use std::iter;

use glam::DVec3;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::{AttributeValue, WorldClock};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_loot_condition::{
    ClockView, EnvironmentView, NbtNumber, ScoreHolder, ScoreView, StorageView, WeatherView,
};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_predicate::{ActiveEffect, EntityView, ItemSubject, LocationView};
use mcrs_minecraft_resource::{Id, ResourceLocation};
use mcrs_voxel::{BlockPos, BlockStateId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mob {
    Pig,
    Zombie,
    Arrow,
}

impl Mob {
    fn type_name(self) -> &'static str {
        match self {
            Mob::Pig => "minecraft:pig",
            Mob::Zombie => "minecraft:zombie",
            Mob::Arrow => "minecraft:arrow",
        }
    }
}

#[derive(Default)]
pub struct Mobs<'a> {
    pub weapon: Option<&'a ItemStack>,
    pub scores: &'a [(Mob, &'a str, i32)],
    pub rain: Option<f32>,
    pub thunder: Option<f32>,
    pub clock: Option<i64>,
    pub environment: &'a [(Option<DVec3>, AttributeValue)],
}

impl LocationView for Mobs<'_> {
    fn block_state(&self, _: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn is_loaded(&self, _: BlockPos) -> bool {
        false
    }

    fn fluid_state(&self, _: BlockPos) -> Option<FluidState> {
        None
    }

    fn block_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light_level(&self) -> Option<f32> {
        None
    }

    fn biome(&self, _: BlockPos) -> Option<Id<Biome>> {
        None
    }

    fn inside_structure(&self, _: BlockPos, _: Id<Structure>) -> Option<bool> {
        None
    }

    fn dimension(&self) -> Option<Id<Dimension>> {
        None
    }

    fn block_entity_data(&self, _: BlockPos, _: &mut NbtCompound) -> bool {
        false
    }
}

impl EntityView for Mobs<'_> {
    type Entity = Mob;

    fn entity_type(&self, mob: Mob) -> Option<Id<EntityType>> {
        let name = ResourceLocation::read(mob.type_name()).ok()?;
        test_registries().entries::<EntityType>()?.by_name(name)
    }

    fn position(&self, _: Mob) -> Option<DVec3> {
        None
    }

    fn is_on_ground(&self, _: Mob) -> Option<bool> {
        None
    }

    fn support_position(&self, _: Mob) -> Option<BlockPos> {
        None
    }

    fn movement_position(&self, _: Mob) -> Option<BlockPos> {
        None
    }

    fn reported_movement(&self, _: Mob) -> Option<DVec3> {
        None
    }

    fn fall_distance(&self, _: Mob) -> Option<f64> {
        None
    }

    fn saved_data(&self, _: Mob, _: &mut NbtCompound) -> bool {
        false
    }

    fn is_burning(&self, _: Mob) -> Option<bool> {
        None
    }

    fn pose(&self, _: Mob) -> Option<Pose> {
        None
    }

    fn is_sprinting(&self, _: Mob) -> Option<bool> {
        None
    }

    fn is_swimming(&self, _: Mob) -> Option<bool> {
        None
    }

    fn is_in_water(&self, _: Mob) -> Option<bool> {
        None
    }

    fn is_living(&self, mob: Mob) -> Option<bool> {
        Some(mob != Mob::Arrow)
    }

    fn is_fall_flying(&self, _: Mob) -> Option<bool> {
        None
    }

    fn has_flying_ability(&self, _: Mob) -> Option<bool> {
        None
    }

    fn is_baby(&self, _: Mob) -> Option<bool> {
        None
    }

    fn active_effect(&self, _: Mob, _: Id<MobEffect>) -> Option<ActiveEffect> {
        None
    }

    fn equipment(&self, _: Mob, slot: EquipmentSlot) -> Option<ItemSubject<'_>> {
        match slot {
            EquipmentSlot::MainHand => {
                Some(self.weapon.map_or(ItemSubject::Empty, ItemSubject::Stack))
            }
            _ => Some(ItemSubject::Empty),
        }
    }

    fn slot(&self, _: Mob, _: u16) -> Option<ItemSubject<'_>> {
        None
    }

    fn tick_count(&self, _: Mob) -> Option<u32> {
        None
    }

    fn vehicle(&self, _: Mob) -> Option<Mob> {
        None
    }

    fn passengers(&self, _: Mob) -> Option<impl Iterator<Item = Mob>> {
        None::<iter::Empty<Mob>>
    }

    fn attack_target(&self, _: Mob) -> Option<Mob> {
        None
    }

    fn team(&self, _: Mob) -> Option<&str> {
        None
    }

    fn has_scoreboard_tag(&self, _: Mob, _: &str) -> Option<bool> {
        None
    }
}

impl ScoreView<Mob> for Mobs<'_> {
    fn score(&self, holder: ScoreHolder<'_, Mob>, objective: &str) -> Option<i32> {
        let ScoreHolder::Entity(&mob) = holder else {
            return None;
        };
        self.scores
            .iter()
            .find(|&&(owner, name, _)| owner == mob && name == objective)
            .map(|&(_, _, score)| score)
    }
}

impl StorageView for Mobs<'_> {
    fn storage_number(&self, _: ResourceLocation<'_>, _: &NbtPath) -> Option<NbtNumber> {
        None
    }
}

impl EnvironmentView for Mobs<'_> {
    fn environment_value(
        &self,
        _: EnvironmentAttribute,
        position: Option<DVec3>,
    ) -> Option<AttributeValue> {
        self.environment
            .iter()
            .find(|(at, _)| *at == position)
            .map(|(_, value)| value.clone())
    }
}

impl WeatherView for Mobs<'_> {
    fn rain_strength(&self) -> Option<f32> {
        self.rain
    }

    fn thunder_strength(&self) -> Option<f32> {
        self.thunder
    }
}

impl ClockView for Mobs<'_> {
    fn total_ticks(&self, _: Id<WorldClock>) -> Option<i64> {
        self.clock
    }
}
