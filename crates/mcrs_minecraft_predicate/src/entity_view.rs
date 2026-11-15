use glam::DVec3;
use mcrs_minecraft_entity::Pose;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

use crate::active_effect::ActiveEffect;

/// The entities as an entity predicate reads them. `None` means that the host does not have the
/// value; the test that reads it fails. A method marked "living" is read only for an entity for
/// which [`EntityView::is_living`] answers `Some(true)`.
pub trait EntityView {
    /// A handle of one entity, valid for the evaluation.
    type Entity: Copy;

    /// The type, as its id in the entity type registry of the registry set of the bake.
    fn entity_type(&self, entity: Self::Entity) -> Option<Id<EntityType>>;

    /// The centre of the bottom face of the bounding box.
    fn position(&self, entity: Self::Entity) -> Option<DVec3>;

    fn is_on_ground(&self, entity: Self::Entity) -> Option<bool>;

    /// The block that carries the entity; read only when the entity is on the ground.
    fn support_position(&self, entity: Self::Entity) -> Option<BlockPos>;

    /// The block whose properties affect the movement of the entity.
    fn movement_position(&self, entity: Self::Entity) -> Option<BlockPos>;

    /// The reported movement in blocks for each tick, by the table of PRED-ENT-011: a player that
    /// rides without control gives its vehicle's, another player the position change of its last
    /// movement packet (zero in a tick without one), a live entity that a player passenger
    /// controls the player's, and each other entity its velocity.
    fn reported_movement(&self, entity: Self::Entity) -> Option<DVec3>;

    fn fall_distance(&self, entity: Self::Entity) -> Option<f64>;

    /// Writes the saved data of the entity into `data`, which the caller passes empty. `false`
    /// when the host does not have the data. For a player, the data also holds the selected stack
    /// under `SelectedItem` when that stack is not empty (PRED-ENT-013).
    fn saved_data(&self, entity: Self::Entity, data: &mut NbtCompound) -> bool;

    fn is_burning(&self, entity: Self::Entity) -> Option<bool>;

    fn pose(&self, entity: Self::Entity) -> Option<Pose>;

    fn is_sprinting(&self, entity: Self::Entity) -> Option<bool>;

    fn is_swimming(&self, entity: Self::Entity) -> Option<bool>;

    fn is_in_water(&self, entity: Self::Entity) -> Option<bool>;

    /// Whether the type of the entity is a living entity type.
    fn is_living(&self, entity: Self::Entity) -> Option<bool>;

    /// Living: whether the entity is in elytra flight.
    fn is_fall_flying(&self, entity: Self::Entity) -> Option<bool>;

    /// Living: the flying ability of a player; `Some(false)` for each other living entity.
    fn has_flying_ability(&self, entity: Self::Entity) -> Option<bool>;

    /// Living: whether the entity is a baby.
    fn is_baby(&self, entity: Self::Entity) -> Option<bool>;

    /// Living: the active effect of the kind `effect`; `None` also when the entity does not
    /// have it.
    fn active_effect(&self, entity: Self::Entity, effect: Id<MobEffect>) -> Option<ActiveEffect>;

    /// The number of ticks that the entity has lived.
    fn tick_count(&self, entity: Self::Entity) -> Option<u32>;

    /// `None` also when the entity rides nothing.
    fn vehicle(&self, entity: Self::Entity) -> Option<Self::Entity>;

    /// The direct passengers, not their passengers.
    fn passengers(&self, entity: Self::Entity) -> Option<impl Iterator<Item = Self::Entity>>;

    /// The attack target of a mob; `None` also for an entity that is not a mob or has no target.
    fn attack_target(&self, entity: Self::Entity) -> Option<Self::Entity>;

    /// The name of the scoreboard team; `None` also for an entity with no team.
    fn team(&self, entity: Self::Entity) -> Option<&str>;

    /// Whether the entity has the scoreboard tag `tag`, which the command `/tag` sets.
    fn has_scoreboard_tag(&self, entity: Self::Entity, tag: &str) -> Option<bool>;
}
