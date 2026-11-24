use glam::DVec3;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::{AttributeValue, WorldClock};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_loot_condition::{
    ClockView, EnvironmentView, NbtNumber, ScoreHolder, ScoreView, StorageView, WeatherView,
};
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::{EntityView, ItemSubject, LocationView};
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
}

impl EntityView for Mobs<'_> {
    type Entity = Mob;

    fn entity_type(&self, mob: Mob) -> Option<Id<EntityType>> {
        let name = ResourceLocation::read(mob.type_name()).ok()?;
        test_registries().entries::<EntityType>()?.by_name(name)
    }

    fn is_living(&self, mob: Mob) -> Option<bool> {
        Some(mob != Mob::Arrow)
    }

    fn equipment(&self, _: Mob, slot: EquipmentSlot) -> Option<ItemSubject<'_>> {
        match slot {
            EquipmentSlot::MainHand => {
                Some(self.weapon.map_or(ItemSubject::Empty, ItemSubject::Stack))
            }
            _ => Some(ItemSubject::Empty),
        }
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
