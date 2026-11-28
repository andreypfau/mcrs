use glam::DVec3;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;

#[must_use]
pub fn mob_body(
    kind: &EntityTypeData,
    row: &MotionRow,
    key: EntityUuid,
    position: DVec3,
) -> EntityState {
    let mut living = Living::of(row);
    if let Some(bases) = kind.attribute_bases()
        && let Some(movement_speed) = bases.base(Attribute::MovementSpeed)
    {
        living.attributes.movement_speed = movement_speed;
    }
    let body = Body {
        position,
        ..Body::default()
    };
    let mut state = EntityState::new(key, body, Some(living));
    state.phase = key.phase_offset();
    state
}
