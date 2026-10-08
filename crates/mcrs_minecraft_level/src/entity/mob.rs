use bevy_derive::Deref;
use bevy_ecs::prelude::{Component, Entity};
use bitflags::bitflags;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_entity::variant::{
    CatSoundVariant, CatVariant as CatVariantValue, ChickenSoundVariant,
    ChickenVariant as ChickenVariantValue, ZombieNautilusVariant as ZombieNautilusVariantValue,
};
use mcrs_minecraft_entity::villager::VillagerData;
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_registry::Id;
use uuid::Uuid;

#[derive(Component, Clone, Copy, Debug, Deref)]
pub struct EntityKind(pub Id<EntityType>);

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

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatVariant {
    pub variant: Id<CatVariantValue>,
    pub sound: Id<CatSoundVariant>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChickenVariant {
    pub variant: Id<ChickenVariantValue>,
    pub sound: Id<ChickenSoundVariant>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZombieNautilusVariant(pub Id<ZombieNautilusVariantValue>);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_components_stay_sixteen_bits_per_id() {
        assert!(size_of::<CatVariant>() <= 4);
        assert!(size_of::<ChickenVariant>() <= 4);
        assert!(size_of::<ZombieNautilusVariant>() <= 2);
    }
}
