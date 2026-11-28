use core::iter;

use glam::DVec3;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

use crate::active_effect::ActiveEffect;
use crate::item_subject::ItemSubject;

pub trait EntityView {
    type Entity: Copy + PartialEq;

    fn entity_type(&self, _entity: Self::Entity) -> Option<Id<EntityType>> {
        None
    }

    fn position(&self, _entity: Self::Entity) -> Option<DVec3> {
        None
    }

    fn is_on_ground(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn support_position(&self, _entity: Self::Entity) -> Option<BlockPos> {
        None
    }

    fn movement_position(&self, _entity: Self::Entity) -> Option<BlockPos> {
        None
    }

    fn reported_movement(&self, _entity: Self::Entity) -> Option<DVec3> {
        None
    }

    fn fall_distance(&self, _entity: Self::Entity) -> Option<f64> {
        None
    }

    fn saved_data(&self, _entity: Self::Entity, _data: &mut NbtCompound) -> bool {
        false
    }

    fn is_burning(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn pose(&self, _entity: Self::Entity) -> Option<Pose> {
        None
    }

    fn is_sprinting(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn is_swimming(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn is_in_water(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn is_living(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn is_fall_flying(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn has_flying_ability(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn is_baby(&self, _entity: Self::Entity) -> Option<bool> {
        None
    }

    fn active_effect(&self, _entity: Self::Entity, _effect: Id<MobEffect>) -> Option<ActiveEffect> {
        None
    }

    fn equipment(&self, _entity: Self::Entity, _slot: EquipmentSlot) -> Option<ItemSubject<'_>> {
        None
    }

    fn slot(&self, _entity: Self::Entity, _slot: u16) -> Option<ItemSubject<'_>> {
        None
    }

    fn tick_count(&self, _entity: Self::Entity) -> Option<u32> {
        None
    }

    fn vehicle(&self, _entity: Self::Entity) -> Option<Self::Entity> {
        None
    }

    fn passengers(&self, _entity: Self::Entity) -> Option<impl Iterator<Item = Self::Entity>> {
        None::<iter::Empty<Self::Entity>>
    }

    fn attack_target(&self, _entity: Self::Entity) -> Option<Self::Entity> {
        None
    }

    fn team(&self, _entity: Self::Entity) -> Option<&str> {
        None
    }

    fn has_scoreboard_tag(&self, _entity: Self::Entity, _tag: &str) -> Option<bool> {
        None
    }
}
