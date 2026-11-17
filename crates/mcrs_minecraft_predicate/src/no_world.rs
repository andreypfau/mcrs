use core::iter;

use glam::DVec3;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::Pose;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::active_effect::ActiveEffect;
use crate::entity_view::EntityView;
use crate::item_subject::ItemSubject;
use crate::location_view::LocationView;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoWorld;

impl LocationView for NoWorld {
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

impl EntityView for NoWorld {
    type Entity = ();

    fn entity_type(&self, (): ()) -> Option<Id<EntityType>> {
        None
    }

    fn position(&self, (): ()) -> Option<DVec3> {
        None
    }

    fn is_on_ground(&self, (): ()) -> Option<bool> {
        None
    }

    fn support_position(&self, (): ()) -> Option<BlockPos> {
        None
    }

    fn movement_position(&self, (): ()) -> Option<BlockPos> {
        None
    }

    fn reported_movement(&self, (): ()) -> Option<DVec3> {
        None
    }

    fn fall_distance(&self, (): ()) -> Option<f64> {
        None
    }

    fn saved_data(&self, (): (), _: &mut NbtCompound) -> bool {
        false
    }

    fn is_burning(&self, (): ()) -> Option<bool> {
        None
    }

    fn pose(&self, (): ()) -> Option<Pose> {
        None
    }

    fn is_sprinting(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_swimming(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_in_water(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_living(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_fall_flying(&self, (): ()) -> Option<bool> {
        None
    }

    fn has_flying_ability(&self, (): ()) -> Option<bool> {
        None
    }

    fn is_baby(&self, (): ()) -> Option<bool> {
        None
    }

    fn active_effect(&self, (): (), _: Id<MobEffect>) -> Option<ActiveEffect> {
        None
    }

    fn equipment(&self, (): (), _: EquipmentSlot) -> Option<ItemSubject<'_>> {
        None
    }

    fn slot(&self, (): (), _: u16) -> Option<ItemSubject<'_>> {
        None
    }

    fn tick_count(&self, (): ()) -> Option<u32> {
        None
    }

    fn vehicle(&self, (): ()) -> Option<()> {
        None
    }

    fn passengers(&self, (): ()) -> Option<impl Iterator<Item = ()>> {
        None::<iter::Empty<()>>
    }

    fn attack_target(&self, (): ()) -> Option<()> {
        None
    }

    fn team(&self, (): ()) -> Option<&str> {
        None
    }

    fn has_scoreboard_tag(&self, (): (), _: &str) -> Option<bool> {
        None
    }
}
