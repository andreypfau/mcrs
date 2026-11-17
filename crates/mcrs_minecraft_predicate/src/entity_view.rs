use glam::DVec3;
use mcrs_minecraft_entity::Pose;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

use crate::active_effect::ActiveEffect;
use crate::item_subject::ItemSubject;

pub trait EntityView {
    type Entity: Copy + PartialEq;

    fn entity_type(&self, entity: Self::Entity) -> Option<Id<EntityType>>;

    fn position(&self, entity: Self::Entity) -> Option<DVec3>;

    fn is_on_ground(&self, entity: Self::Entity) -> Option<bool>;

    fn support_position(&self, entity: Self::Entity) -> Option<BlockPos>;

    fn movement_position(&self, entity: Self::Entity) -> Option<BlockPos>;

    fn reported_movement(&self, entity: Self::Entity) -> Option<DVec3>;

    fn fall_distance(&self, entity: Self::Entity) -> Option<f64>;

    fn saved_data(&self, entity: Self::Entity, data: &mut NbtCompound) -> bool;

    fn is_burning(&self, entity: Self::Entity) -> Option<bool>;

    fn pose(&self, entity: Self::Entity) -> Option<Pose>;

    fn is_sprinting(&self, entity: Self::Entity) -> Option<bool>;

    fn is_swimming(&self, entity: Self::Entity) -> Option<bool>;

    fn is_in_water(&self, entity: Self::Entity) -> Option<bool>;

    fn is_living(&self, entity: Self::Entity) -> Option<bool>;

    fn is_fall_flying(&self, entity: Self::Entity) -> Option<bool>;

    fn has_flying_ability(&self, entity: Self::Entity) -> Option<bool>;

    fn is_baby(&self, entity: Self::Entity) -> Option<bool>;

    fn active_effect(&self, entity: Self::Entity, effect: Id<MobEffect>) -> Option<ActiveEffect>;

    fn equipment(&self, entity: Self::Entity, slot: EquipmentSlot) -> Option<ItemSubject<'_>>;

    fn slot(&self, entity: Self::Entity, slot: u16) -> Option<ItemSubject<'_>>;

    fn tick_count(&self, entity: Self::Entity) -> Option<u32>;

    fn vehicle(&self, entity: Self::Entity) -> Option<Self::Entity>;

    fn passengers(&self, entity: Self::Entity) -> Option<impl Iterator<Item = Self::Entity>>;

    fn attack_target(&self, entity: Self::Entity) -> Option<Self::Entity>;

    fn team(&self, entity: Self::Entity) -> Option<&str>;

    fn has_scoreboard_tag(&self, entity: Self::Entity, tag: &str) -> Option<bool>;
}
