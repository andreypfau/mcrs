use bevy_derive::Deref;
use bevy_ecs::prelude::{Component, Entity};
use bitflags::bitflags;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_world::entity::EntityType;
use mcrs_minecraft_world::entity::villager::VillagerData;
use uuid::Uuid;

#[derive(Component, Clone, Copy, Debug, Deref)]
pub struct EntityKind(pub &'static EntityType);

#[derive(Debug, Clone, Copy, Component, Deref, PartialEq, Eq)]
pub struct EntityUuid(pub Uuid);

impl Default for EntityUuid {
    fn default() -> Self {
        EntityUuid(Uuid::new_v4())
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn full(max: f32) -> Self {
        Self { current: max, max }
    }
}

bitflags! {
    #[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct MobFlags: u8 {
        const NO_AI = 1;
        const LEFT_HANDED = 2;
        const AGGRESSIVE = 4;
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Equipment {
    pub mainhand: Option<ItemStack>,
    pub offhand: Option<ItemStack>,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Baby;

/// Registry ids, as the wire carries them.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatVariant {
    pub variant: u32,
    pub sound: u32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChickenVariant {
    pub variant: u32,
    pub sound: u32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZombieNautilusVariant(pub u32);

#[derive(Component, Clone, Copy, Debug, Deref, PartialEq, Eq)]
pub struct Villager(pub VillagerData);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemFrame {
    pub item: Option<ItemStack>,
    pub facing: Direction,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ContainerLoot {
    pub table: String,
    pub seed: i64,
}

#[derive(Component, Clone, Copy, Debug, Deref)]
#[relationship(relationship_target = RiddenBy)]
pub struct Riding(pub Entity);

#[derive(Component, Debug, Default, Deref)]
#[relationship_target(relationship = Riding)]
pub struct RiddenBy(Vec<Entity>);

/// Back-link from a mob to the section holding it; the section's despawn
/// takes its mobs with it.
#[derive(Component, Clone, Copy, Debug, Deref)]
#[relationship(relationship_target = SectionMobs)]
pub struct EntityInSection(pub Entity);

#[derive(Component, Debug, Default, Deref)]
#[relationship_target(relationship = EntityInSection, linked_spawn)]
pub struct SectionMobs(Vec<Entity>);
